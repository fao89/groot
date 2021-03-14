CREATE TABLE collections (
  id SERIAL PRIMARY KEY,
  namespace VARCHAR NOT NULL,
  name VARCHAR NOT NULL,
  UNIQUE (namespace, name)
)
