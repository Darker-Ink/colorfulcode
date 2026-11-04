// Find friends of friends who like the same movies
MATCH (me:Person {name: $name})-[:KNOWS*1..2]->(fof:Person)
WHERE fof <> me
  AND NOT (me)-[:KNOWS]->(fof)
  AND fof.age >= 18
WITH me, fof, count(*) AS strength
OPTIONAL MATCH (fof)-[r:LIKES|LOVES]->(m:Movie)
WHERE m.released > 2000 AND m.title STARTS WITH 'The'
RETURN fof.name AS name,
       strength,
       collect(DISTINCT m.title)[0..5] AS movies,
       CASE WHEN strength > 3 THEN "close" ELSE "distant" END AS bond
ORDER BY strength DESC
LIMIT 25;

/* Create some data */
CREATE (a:Person:Admin {name: 'Ada', born: 1815, active: true})
CREATE (b:`Movie Star` {name: "Grace \"Amazing\" Hopper", rating: 9.5})
MERGE (a)-[rel:KNOWS {since: date('2020-01-01')}]->(b)
ON CREATE SET rel.created = timestamp()
ON MATCH SET rel.seen = rel.seen + 1;

UNWIND range(1, 10) AS i
CALL {
  WITH i
  MATCH (n:Node {id: i})
  DETACH DELETE n
} IN TRANSACTIONS OF 100 ROWS;

match (n) where n.deleted is null return toUpper(n.name), labels(n), size(keys(n));
CREATE CONSTRAINT person_name IF NOT EXISTS FOR (p:Person) REQUIRE p.name IS UNIQUE;
