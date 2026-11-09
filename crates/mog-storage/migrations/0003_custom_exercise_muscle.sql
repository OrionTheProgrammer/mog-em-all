-- Músculo principal de los ejercicios propios (los incluidos lo toman del catálogo).
-- Mismo número que catalog::Muscle::index en mog-core.
ALTER TABLE exercise ADD COLUMN muscle INTEGER;
