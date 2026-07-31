CREATE TABLE IF NOT EXISTS holdings (
 user_id BIGINT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
 asset_id BIGINT NOT NULL REFERENCES assets (id) ON DELETE CASCADE,
 quantity DOUBLE PRECISION NOT NULL CHECK (quantity > 0),
 PRIMARY KEY (user_id, asset_id)
);

CREATE INDEX IF NOT EXISTS holdings_user_id_idx ON holdings (user_id);
