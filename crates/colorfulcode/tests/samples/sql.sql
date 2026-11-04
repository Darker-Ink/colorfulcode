-- Schema for a small shop
CREATE TABLE IF NOT EXISTS customers (
    id          BIGSERIAL PRIMARY KEY,
    "Full Name" VARCHAR(255) NOT NULL,
    email       TEXT UNIQUE,
    balance     NUMERIC(10, 2) DEFAULT 0.00,
    created_at  TIMESTAMPTZ DEFAULT now()
);

/* Find the best customers
   in the last 30 days */
SELECT c.id,
       c."Full Name" AS name,
       COUNT(o.id) AS orders,
       SUM(o.total)::numeric AS spent
FROM customers c
LEFT JOIN orders o ON o.customer_id = c.id
WHERE o.created_at > NOW() - INTERVAL '30 days'
  AND c.email LIKE '%@example.com'
  AND c.note <> 'it''s complicated'
GROUP BY c.id
HAVING COUNT(o.id) >= 3
ORDER BY spent DESC NULLS LAST
LIMIT 10 OFFSET :page;

INSERT INTO customers (email, balance) VALUES ($1, $2) RETURNING id;
UPDATE `inventory` SET qty = qty - 1 WHERE sku = @sku;
SELECT [Order Details].[Unit Price] FROM [Order Details];

CREATE OR REPLACE FUNCTION add_points(uid int, pts int) RETURNS void AS $$
BEGIN
    UPDATE customers SET balance = balance + pts WHERE id = uid;
END;
$$ LANGUAGE plpgsql;

select count(*) from users where active = true and deleted_at is null;
