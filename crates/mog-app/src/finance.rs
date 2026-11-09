//! Pestaña FINANZAS: movimientos del mes, grupos de gasto con presupuesto y fijos.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use chrono::{Datelike, Local, NaiveDate};
use mog_core::finance::{self, BudgetState, Currency, EntryKind, IncomeType};
use mog_core::schedule::{self, DayNumber};
use mog_core::tasks::{civil_from_days, days_from_civil, days_in_month};
use mog_storage::{Category, CategoryId, Entry, Recurring, Store};
use mog_ui::{AppWindow, CategoryRow, EntryRow, FinanceState, FixedRow};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

type SharedStore = Rc<RefCell<Store>>;

/// Nombres de los grupos incluidos en los dos idiomas, para encontrarlos con `#`.
const CATEGORY_NAMES: &[(&str, &[&str])] = &[
    ("food", &["comida", "food"]),
    ("transport", &["transporte", "transport"]),
    ("home", &["casa", "hogar", "home"]),
    ("bills", &["cuentas", "servicios", "bills"]),
    ("health", &["salud", "health"]),
    ("fun", &["salidas", "ocio", "fun"]),
    ("education", &["educacion", "estudios", "education"]),
    ("subscriptions", &["suscripciones", "subscriptions"]),
    ("gym", &["gimnasio", "gym"]),
    ("other", &["otros", "other"]),
];

#[derive(Default)]
struct View {
    month_offset: Cell<i64>,
}

struct Ctx {
    ui: slint::Weak<AppWindow>,
    store: SharedStore,
    view: Rc<View>,
}

impl Ctx {
    fn run(&self, action: impl FnOnce(&AppWindow, &mut Store, &View)) {
        if let Some(ui) = self.ui.upgrade() {
            let mut store = self.store.borrow_mut();
            action(&ui, &mut store, &self.view);
            refresh_with(&ui, &store, &self.view);
        }
    }
}

/// Conecta la pestaña y devuelve una función para refrescarla desde otros módulos.
pub fn connect(ui: &AppWindow, store: &SharedStore) -> impl Fn() + 'static {
    let view = Rc::new(View::default());
    let ctx = Rc::new(Ctx {
        ui: ui.as_weak(),
        store: store.clone(),
        view: view.clone(),
    });
    let state = ui.global::<FinanceState>();

    state.on_prev_month({
        let ctx = ctx.clone();
        move || ctx.run(|_, _, view| view.month_offset.set(view.month_offset.get() - 1))
    });
    state.on_next_month({
        let ctx = ctx.clone();
        move || ctx.run(|_, _, view| view.month_offset.set(view.month_offset.get() + 1))
    });
    state.on_this_month({
        let ctx = ctx.clone();
        move || ctx.run(|_, _, view| view.month_offset.set(0))
    });
    state.on_quick_changed({
        let ctx = ctx.clone();
        move || ctx.run(|_, _, _| {})
    });
    state.on_quick_add({
        let ctx = ctx.clone();
        move || {
            let mut added = false;
            ctx.run(|ui, store, _| {
                let state = ui.global::<FinanceState>();
                let currency = currency_of(store);
                let categories = store.categories().unwrap_or_default();
                let Some(quick) = finance::parse_quick(&state.get_quick_text(), currency, today())
                else {
                    return;
                };
                let entry = Entry {
                    kind: quick.kind,
                    amount: quick.amount,
                    category_id: resolve_category(&quick, &categories),
                    income_type: (quick.kind == EntryKind::Income).then_some(IncomeType::Informal),
                    note: quick.note,
                    day: quick.day,
                    ..Default::default()
                };
                added = report(store.add_entry(&entry, crate::now())).is_some();
                if added {
                    state.set_quick_text(SharedString::new());
                }
            });
            added
        }
    });
    state.on_delete_entry({
        let ctx = ctx.clone();
        move |id| {
            ctx.run(|_, store, _| {
                report(store.delete_entry(i64::from(id)));
            })
        }
    });
    state.on_select_category({
        let ctx = ctx.clone();
        move |id| {
            ctx.run(|ui, store, _| {
                let state = ui.global::<FinanceState>();
                let Some(category) = store
                    .categories()
                    .unwrap_or_default()
                    .into_iter()
                    .find(|c| c.id == i64::from(id))
                else {
                    return;
                };
                let currency = currency_of(store);
                state.set_budget_id(id);
                state.set_budget_name(category_label(&category, ui).into());
                state.set_budget_text(
                    category
                        .budget
                        .map(|b| plain_amount(b, currency))
                        .unwrap_or_default()
                        .into(),
                );
                state.set_budget_error(false);
            })
        }
    });
    state.on_save_budget({
        let ctx = ctx.clone();
        move || {
            ctx.run(|ui, store, _| {
                let state = ui.global::<FinanceState>();
                let text = state.get_budget_text();
                let budget = if text.trim().is_empty() {
                    None
                } else {
                    match finance::parse_amount(&text, currency_of(store)) {
                        Some(b) => Some(b),
                        None => {
                            state.set_budget_error(true);
                            return;
                        }
                    }
                };
                report(store.set_category_budget(i64::from(state.get_budget_id()), budget));
                state.set_budget_id(0);
            })
        }
    });
    state.on_close_budget({
        let ctx = ctx.clone();
        move || ctx.run(|ui, _, _| ui.global::<FinanceState>().set_budget_id(0))
    });
    state.on_add_category({
        let ctx = ctx.clone();
        move |name| {
            ctx.run(|_, store, _| {
                let name = name.trim();
                if !name.is_empty() && name.chars().count() <= 40 {
                    report(store.add_category(name));
                }
            })
        }
    });
    state.on_register_fixed({
        let ctx = ctx.clone();
        move |id| {
            ctx.run(|_, store, view| {
                let Some(item) = store
                    .recurring()
                    .unwrap_or_default()
                    .into_iter()
                    .find(|r| r.id == i64::from(id))
                else {
                    return;
                };
                let (first, last) = month_range(view);
                // El día que toca en el mes mostrado; si todavía no llega, se anota hoy.
                let due = (first + i64::from(item.day_of_month) - 1).min(last);
                let day = if due > today() && today() >= first {
                    today()
                } else {
                    due
                };
                report(store.register_recurring(item.id, day, crate::now()));
            })
        }
    });
    state.on_new_fixed({
        let ctx = ctx.clone();
        move |kind| {
            ctx.run(|ui, store, _| {
                let state = ui.global::<FinanceState>();
                state.set_fixed_id(0);
                state.set_fixed_kind(kind);
                state.set_fixed_amount(SharedString::new());
                state.set_fixed_note(SharedString::new());
                state.set_fixed_day("1".into());
                let other = store
                    .categories()
                    .unwrap_or_default()
                    .into_iter()
                    .find(|c| c.code.as_deref() == Some("subscriptions"));
                state.set_fixed_category(other.map_or(0, |c| c.id as i32));
                state.set_fixed_error(0);
                state.set_fixed_open(true);
            })
        }
    });
    state.on_edit_fixed({
        let ctx = ctx.clone();
        move |id| {
            ctx.run(|ui, store, _| {
                let Some(item) = store
                    .recurring()
                    .unwrap_or_default()
                    .into_iter()
                    .find(|r| r.id == i64::from(id))
                else {
                    return;
                };
                let state = ui.global::<FinanceState>();
                state.set_fixed_id(id);
                state.set_fixed_kind(if item.kind == EntryKind::Income { 1 } else { 0 });
                state.set_fixed_amount(plain_amount(item.amount, currency_of(store)).into());
                state.set_fixed_note(item.note.into());
                state.set_fixed_day(item.day_of_month.to_string().into());
                state.set_fixed_category(item.category_id.map_or(0, |c| c as i32));
                state.set_fixed_error(0);
                state.set_fixed_open(true);
            })
        }
    });
    state.on_save_fixed({
        let ctx = ctx.clone();
        move || {
            ctx.run(|ui, store, _| {
                let state = ui.global::<FinanceState>();
                let Some(amount) =
                    finance::parse_amount(&state.get_fixed_amount(), currency_of(store))
                else {
                    return state.set_fixed_error(1);
                };
                let note = state.get_fixed_note().trim().to_string();
                if note.is_empty() {
                    return state.set_fixed_error(2);
                }
                let Some(day) = state
                    .get_fixed_day()
                    .trim()
                    .parse::<u32>()
                    .ok()
                    .filter(|d| (1..=31).contains(d))
                else {
                    return state.set_fixed_error(3);
                };
                let kind = if state.get_fixed_kind() == 1 {
                    EntryKind::Income
                } else {
                    EntryKind::Expense
                };
                let item = Recurring {
                    id: i64::from(state.get_fixed_id()),
                    kind,
                    amount,
                    category_id: Some(i64::from(state.get_fixed_category())).filter(|c| *c != 0),
                    note,
                    day_of_month: day,
                };
                if report(store.save_recurring(&item, crate::now())).is_some() {
                    state.set_fixed_open(false);
                }
            })
        }
    });
    state.on_delete_fixed({
        let ctx = ctx.clone();
        move || {
            ctx.run(|ui, store, _| {
                let state = ui.global::<FinanceState>();
                report(store.delete_recurring(i64::from(state.get_fixed_id())));
                state.set_fixed_open(false);
            })
        }
    });

    refresh_with(ui, &store.borrow(), &view);
    move || ctx.run(|_, _, _| {})
}

fn refresh_with(ui: &AppWindow, store: &Store, view: &View) {
    let state = ui.global::<FinanceState>();
    let currency = currency_of(store);
    let fmt = |minor: i64| format_amount(ui, minor, currency);
    let (first, last) = month_range(view);
    let (year, month, _) = civil_from_days(first);
    state.set_year(year as i32);
    state.set_month(month as i32);
    state.set_month_offset(view.month_offset.get() as i32);
    state.set_currency(currency.code.into());

    let categories = store.categories().unwrap_or_default();
    let by_id: HashMap<CategoryId, &Category> = categories.iter().map(|c| (c.id, c)).collect();
    let entries = store.entries_between(first, last).unwrap_or_default();

    let sum = |f: &dyn Fn(&Entry) -> bool| {
        entries
            .iter()
            .filter(|e| f(e))
            .map(|e| e.amount)
            .sum::<i64>()
    };
    let fixed_income =
        sum(&|e| e.kind == EntryKind::Income && e.income_type == Some(IncomeType::Fixed));
    let other_income =
        sum(&|e| e.kind == EntryKind::Income && e.income_type != Some(IncomeType::Fixed));
    let spent = sum(&|e| e.kind == EntryKind::Expense);
    state.set_income_text(fmt(fixed_income + other_income).into());
    state.set_fixed_income_text(fmt(fixed_income).into());
    state.set_informal_income_text(fmt(other_income).into());
    state.set_expense_text(fmt(spent).into());
    let balance = fixed_income + other_income - spent;
    state.set_balance_text(fmt(balance).into());
    state.set_balance_negative(balance < 0);

    let mut budgeted_spent = 0;
    let mut budget_total = 0;
    let rows: Vec<CategoryRow> = categories
        .iter()
        .map(|c| {
            let spent = sum(&|e| e.kind == EntryKind::Expense && e.category_id == Some(c.id));
            if let Some(b) = c.budget {
                budgeted_spent += spent;
                budget_total += b;
            }
            let state = match finance::budget_state(spent, c.budget) {
                BudgetState::NoBudget => 0,
                BudgetState::Fine => 1,
                BudgetState::Warning => 2,
                BudgetState::Over => 3,
            };
            CategoryRow {
                id: c.id as i32,
                code: c.code.clone().unwrap_or_default().into(),
                name: c.name.clone().unwrap_or_default().into(),
                spent_text: fmt(spent).into(),
                budget_text: c.budget.map(fmt).unwrap_or_default().into(),
                ratio: c.budget.map_or(0.0, |b| spent as f32 / b as f32),
                state,
                has_spending: spent > 0,
            }
        })
        .collect();
    state.set_categories(model(rows));
    state.set_budget_used_text(if budget_total > 0 {
        format!("{} %", budgeted_spent * 100 / budget_total).into()
    } else {
        "—".into()
    });

    state.set_entries(model(
        entries.iter().map(|e| entry_row(e, &by_id, &fmt)).collect(),
    ));

    let fixed: Vec<FixedRow> = store
        .recurring()
        .unwrap_or_default()
        .into_iter()
        .map(|r| {
            let category = r.category_id.and_then(|id| by_id.get(&id));
            FixedRow {
                id: r.id as i32,
                kind: if r.kind == EntryKind::Income { 1 } else { 0 },
                amount_text: fmt(r.amount).into(),
                note: r.note.clone().into(),
                day_of_month: r.day_of_month as i32,
                category_code: category
                    .and_then(|c| c.code.clone())
                    .unwrap_or_default()
                    .into(),
                category_name: category
                    .and_then(|c| c.name.clone())
                    .unwrap_or_default()
                    .into(),
                registered: entries.iter().any(|e| e.recurring_id == Some(r.id)),
            }
        })
        .collect();
    state.set_fixed(model(fixed));

    // Vista previa de la entrada rápida.
    match finance::parse_quick(&state.get_quick_text(), currency, today()) {
        Some(quick) => {
            let entry = Entry {
                kind: quick.kind,
                amount: quick.amount,
                category_id: resolve_category(&quick, &categories),
                note: quick.note.clone(),
                day: quick.day,
                ..Default::default()
            };
            state.set_preview(entry_row(&entry, &by_id, &fmt));
            state.set_preview_valid(true);
        }
        None => state.set_preview_valid(false),
    }
}

fn entry_row(
    e: &Entry,
    by_id: &HashMap<CategoryId, &Category>,
    fmt: &dyn Fn(i64) -> String,
) -> EntryRow {
    let category = e.category_id.and_then(|id| by_id.get(&id));
    let date = date_of(e.day);
    EntryRow {
        id: e.id as i32,
        kind: if e.kind == EntryKind::Income { 1 } else { 0 },
        amount_text: fmt(e.amount).into(),
        note: e.note.clone().into(),
        category_code: category
            .and_then(|c| c.code.clone())
            .unwrap_or_default()
            .into(),
        category_name: category
            .and_then(|c| c.name.clone())
            .unwrap_or_default()
            .into(),
        income_type: if e.income_type == Some(IncomeType::Fixed) {
            0
        } else {
            1
        },
        day: date.day() as i32,
        month: date.month() as i32,
        weekday: i32::from(schedule::weekday_of(e.day)),
        from_fixed: e.recurring_id.is_some(),
    }
}

/// Grupo de un gasto: el de `#nombre` si existe, si no el deducido de las palabras.
fn resolve_category(quick: &finance::QuickEntry, categories: &[Category]) -> Option<CategoryId> {
    if quick.kind == EntryKind::Income {
        return None;
    }
    let by_code = |code: &str| {
        categories
            .iter()
            .find(|c| c.code.as_deref() == Some(code))
            .map(|c| c.id)
    };
    if let Some(tag) = &quick.tag {
        let tag = fold(tag);
        let named = categories
            .iter()
            .find(|c| c.name.as_deref().is_some_and(|n| fold(n) == tag))
            .map(|c| c.id);
        let builtin = CATEGORY_NAMES
            .iter()
            .find(|(code, names)| *code == tag || names.contains(&tag.as_str()))
            .and_then(|(code, _)| by_code(code));
        if let Some(id) = named.or(builtin) {
            return Some(id);
        }
    }
    quick.guessed.and_then(by_code).or_else(|| by_code("other"))
}

fn category_label(category: &Category, ui: &AppWindow) -> String {
    match (&category.name, &category.code) {
        (Some(name), _) => name.clone(),
        (None, Some(code)) => {
            let names = CATEGORY_NAMES
                .iter()
                .find(|(c, _)| c == code)
                .map(|(_, n)| *n)
                .unwrap_or(&[]);
            let spanish = ui.get_language() == "es";
            let pick = if spanish { names.first() } else { names.last() };
            pick.map(|n| capitalize(n)).unwrap_or_else(|| code.clone())
        }
        (None, None) => String::new(),
    }
}

fn capitalize(text: &str) -> String {
    let mut chars = text.chars();
    chars
        .next()
        .map(|c| c.to_uppercase().collect::<String>() + chars.as_str())
        .unwrap_or_default()
}

fn fold(text: &str) -> String {
    text.trim()
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'á' => 'a',
            'é' => 'e',
            'í' => 'i',
            'ó' => 'o',
            'ú' | 'ü' => 'u',
            'ñ' => 'n',
            other => other,
        })
        .collect()
}

/// La moneda elegida en Ajustes (por defecto, peso chileno).
pub fn currency_of(store: &Store) -> Currency {
    finance::currency(
        &store
            .setting("currency")
            .ok()
            .flatten()
            .unwrap_or_else(|| "CLP".into()),
    )
}

/// Monto con los separadores del idioma de la interfaz.
fn format_amount(ui: &AppWindow, minor: i64, currency: Currency) -> String {
    let (thousands, decimal) = if ui.get_language() == "es" {
        ('.', ',')
    } else {
        (',', '.')
    };
    finance::format_amount(minor, currency, thousands, decimal)
}

/// Monto para un campo de texto, sin símbolo ni separador de miles ("12500", "12.5").
fn plain_amount(minor: i64, currency: Currency) -> String {
    if currency.decimals == 0 {
        return minor.to_string();
    }
    let factor = 10i64.pow(u32::from(currency.decimals));
    format!(
        "{}.{:0width$}",
        minor / factor,
        minor % factor,
        width = usize::from(currency.decimals)
    )
}

fn month_range(view: &View) -> (DayNumber, DayNumber) {
    let (y, m, _) = civil_from_days(today());
    let total = y * 12 + i64::from(m) - 1 + view.month_offset.get();
    let (year, month) = (total.div_euclid(12), (total.rem_euclid(12) + 1) as u32);
    let first = days_from_civil(year, month, 1);
    (first, first + i64::from(days_in_month(year, month)) - 1)
}

fn today() -> DayNumber {
    (Local::now().date_naive() - epoch()).num_days()
}

fn epoch() -> NaiveDate {
    NaiveDate::from_ymd_opt(1970, 1, 1).expect("fecha válida")
}

fn date_of(day: DayNumber) -> NaiveDate {
    epoch() + chrono::Duration::days(day)
}

fn model<T: Clone + 'static>(items: Vec<T>) -> ModelRc<T> {
    ModelRc::new(VecModel::from(items))
}

fn report<T>(result: mog_storage::Result<T>) -> Option<T> {
    result.map_err(|err| eprintln!("base de datos: {err}")).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn categories() -> Vec<Category> {
        let mut list: Vec<Category> = finance::DEFAULT_CATEGORIES
            .iter()
            .enumerate()
            .map(|(i, code)| Category {
                id: i as i64 + 1,
                code: Some((*code).into()),
                name: None,
                budget: None,
            })
            .collect();
        list.push(Category {
            id: 50,
            code: None,
            name: Some("Mascotas".into()),
            budget: None,
        });
        list
    }

    fn quick(text: &str) -> finance::QuickEntry {
        finance::parse_quick(text, finance::CURRENCIES[0], 100).unwrap()
    }

    #[test]
    fn categories_by_tag_word_or_fallback() {
        let list = categories();
        assert_eq!(
            resolve_category(&quick("12.500 almuerzo"), &list),
            Some(1),
            "comida, por la palabra"
        );
        assert_eq!(
            resolve_category(&quick("5000 cosas #transporte"), &list),
            Some(2),
            "por nombre en español"
        );
        assert_eq!(
            resolve_category(&quick("5000 croquetas #mascotas"), &list),
            Some(50),
            "grupo propio"
        );
        assert_eq!(
            resolve_category(&quick("5000 algo raro"), &list),
            Some(10),
            "sin pistas: otros"
        );
        assert_eq!(
            resolve_category(&quick("+5000 venta"), &list),
            None,
            "los ingresos no tienen grupo"
        );
    }

    #[test]
    fn plain_amounts_for_fields() {
        assert_eq!(plain_amount(12_500, finance::CURRENCIES[0]), "12500");
        assert_eq!(plain_amount(1_250, finance::CURRENCIES[1]), "12.50");
    }
}
