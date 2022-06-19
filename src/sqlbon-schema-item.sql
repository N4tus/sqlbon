CREATE TABLE Item (
    id INTEGER PRIMARY KEY,
    name VARCHAR NOT NULL,
    quantity INT NOT NULL DEFAULT 1,
    price INT NOT NULL,
    unit VARCHAR(3) NOT NULL,
    receipt INT NOT NULL,
    FOREIGN KEY(receipt) REFERENCES Receipt(id)
);
