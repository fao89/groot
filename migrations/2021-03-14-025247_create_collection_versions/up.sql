CREATE TABLE collection_versions (
  id SERIAL PRIMARY KEY,
  collection_id INTEGER NOT NULL,
  version VARCHAR NOT NULL,
  metadata json NOT NULL,
  FOREIGN KEY (collection_id) REFERENCES collections(id),
  UNIQUE (collection_id, version)
)
