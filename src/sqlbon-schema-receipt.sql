CREATE TABLE Receipt (
    id INTEGER PRIMARY KEY,
    store INT NOT NULL,
    date DATE NOT NULL,
    FOREIGN KEY(store) REFERENCES Store(id)
);