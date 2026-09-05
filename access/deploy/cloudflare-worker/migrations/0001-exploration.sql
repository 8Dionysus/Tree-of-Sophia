-- Disposable query state, separate from every source/read-model table.
-- Apply after the read-model tables exist, never as request-time DDL.
CREATE TABLE IF NOT EXISTS knowledge_exploration_clock (
  singleton INTEGER PRIMARY KEY CHECK(singleton = 1), epoch INTEGER NOT NULL
);
INSERT OR IGNORE INTO knowledge_exploration_clock VALUES (1, 0);
CREATE TABLE IF NOT EXISTS knowledge_exploration_checkpoints (
  token TEXT PRIMARY KEY, expires INTEGER NOT NULL, epoch INTEGER NOT NULL,
  version TEXT NOT NULL, state TEXT, response TEXT, successor TEXT,
  bytes INTEGER NOT NULL CHECK(bytes <= 1048576)
);
CREATE INDEX IF NOT EXISTS knowledge_exploration_expiry ON knowledge_exploration_checkpoints(expires);
CREATE INDEX IF NOT EXISTS knowledge_relations_from_seek ON knowledge_relations(from_id, id);
CREATE INDEX IF NOT EXISTS knowledge_relations_to_seek ON knowledge_relations(to_id, id);
CREATE TRIGGER IF NOT EXISTS knowledge_exploration_revision_insert AFTER INSERT ON edge_meta
WHEN NEW.key = 'data_revision' BEGIN
  UPDATE knowledge_exploration_clock SET epoch = epoch + 1 WHERE singleton = 1;
END;
CREATE TRIGGER IF NOT EXISTS knowledge_exploration_revision_update AFTER UPDATE ON edge_meta
WHEN NEW.key = 'data_revision' OR OLD.key = 'data_revision' BEGIN
  UPDATE knowledge_exploration_clock SET epoch = epoch + 1 WHERE singleton = 1;
END;
CREATE TRIGGER IF NOT EXISTS knowledge_exploration_revision_delete AFTER DELETE ON edge_meta
WHEN OLD.key = 'data_revision' BEGIN
  UPDATE knowledge_exploration_clock SET epoch = epoch + 1 WHERE singleton = 1;
END;
