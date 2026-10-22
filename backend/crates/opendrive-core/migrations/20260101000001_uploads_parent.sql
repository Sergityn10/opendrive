-- Amplía los uploads en curso con la carpeta de destino y el tipo MIME.
--
-- El destino y el tipo se fijan al crear el upload (POST) y se usan cuando el
-- archivo se completa, para insertar el nodo final en el árbol.

ALTER TABLE uploads
    ADD COLUMN IF NOT EXISTS parent_id UUID REFERENCES nodes (id) ON DELETE CASCADE,
    ADD COLUMN IF NOT EXISTS mime_type TEXT;

CREATE INDEX IF NOT EXISTS uploads_parent_idx ON uploads (parent_id);
