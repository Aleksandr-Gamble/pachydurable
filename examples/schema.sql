

CREATE TABLE IF NOT EXISTS animals (
    id SERIAL NOT NULL PRIMARY KEY,
    name VARCHAR NOT NULL UNIQUE,
    description VARCHAR,
    autocomp_tsv tsvector GENERATED ALWAYS AS (to_tsvector('simple', name )) STORED,
    fulltext_tsv tsvector GENERATED ALWAYS AS (to_tsvector('english', name || ' ' || description )) STORED
);
CREATE INDEX autocomp_animals ON animals USING GIN(autocomp_tsv);
CREATE INDEX fulltext_animals ON animals USING GIN(fulltext_tsv);

INSERT INTO animals (name, description) VALUES 
('cat', 'soft, fuzzy, knocks things off tables'),
('dog', 'loyal, protective, chases squirrels '),
('fish', 'has scales, is pretty good at swimming'),
('emu', 'big, intimidating birds')
ON CONFLICT (name) DO NOTHING;


CREATE TABLE IF NOT EXISTS foods (
    name VARCHAR NOT NULL PRIMARY KEY,
    color VARCHAR,
    autocomp_tsv tsvector GENERATED ALWAYS AS (to_tsvector('simple', name )) STORED
);
CREATE INDEX autocomp_foods ON foods USING GIN(autocomp_tsv);


INSERT INTO foods (name, color) VALUES 
('apple', NULL),
('strawberry', 'red'),
('kale', 'green'),
('fish', NULL),
('chocolate', 'brown')
ON CONFLICT (name) DO NOTHING;
