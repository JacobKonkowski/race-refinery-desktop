# AI use disclosure

Race Refinery uses AI tools as part of building and shipping the project. This note describes where that happens.

## Software development

AI assistants help with coding and engineering work, including:

- Drafting and editing application code, tests, and scripts
- Exploring the codebase and proposing focused changes
- Writing and updating documentation

People review and merge the result. AI assistance does not replace review, testing, or maintainer judgment.

## Graphic and brand assets

Some graphic and brand assets (icons, illustrations, marketing or UI imagery, and similar visuals) may be created or refined with generative AI tools. Assets are selected and edited before they are used in the product or materials.

## Product behavior

The Race Refinery app itself does **not** call external AI services for telemetry analysis, coaching, or voice. Insights and coach callouts are local and deterministic. See [docs/PRIVACY.md](docs/PRIVACY.md) and [docs/SETUP.md](docs/SETUP.md).
