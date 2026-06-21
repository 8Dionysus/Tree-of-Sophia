# Master Tables

`master-tables/` holds the prepared row spine for ToS philosophy.

The three tables are already formed enough to seed the atlas:

| Table | Rows | File |
| --- | ---: | --- |
| `table-i/` | 48 | `Таблица I для ToS.docx` |
| `table-ii/` | 58 | `Таблица II для ToS.docx` |
| `table-iii/` | 84 | `Таблица III ToS.docx` |

Each table directory owns:

| Surface | Role |
| --- | --- |
| `rows.jsonl` | stable row entries extracted from the prepared master table |
| `table.manifest.json` | table identity, row count, source document name, and dossier coverage |
| `branch.manifest.json` | atlas branch identity for topology validation |

Rows are project atlas material. They route future growth while historical
witnesses live in branch/source surfaces.
