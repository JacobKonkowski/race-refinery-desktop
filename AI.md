# AI use disclosure

Race Refinery uses AI tools as part of building and shipping the project. This note describes where that happens.

## Software development

AI assistants help with coding and engineering work, including:

- Drafting and editing application code, tests, and scripts
- Exploring the codebase and proposing focused changes
- Writing and updating documentation

People review and merge the result. AI assistance does not replace review, testing, or maintainer judgment.

## Product behavior

The Race Refinery app itself does **not** call external AI services for telemetry analysis, coaching, or voice. Insights and coach callouts are local and deterministic. See [docs/PRIVACY.md](docs/PRIVACY.md) and [docs/SETUP.md](docs/SETUP.md).

### Optional MCP bridge (off by default)

Settings can enable a **localhost-only, read-only MCP server** so an external agent host
(such as Cursor) can query imported session and lap data. Race Refinery still does not
embed an LLM or send data to the cloud for that path; enablement and the listen URL are
shown in Settings. Details: [docs/PRIVACY.md](docs/PRIVACY.md).
