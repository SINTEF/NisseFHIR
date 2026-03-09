# Lightweight Rust FHIR server instruction book.

Your task is to build a FHIR 6.0 server. It's a version currently in development, but the working drafts are good enough to get started.

FHIR being a big standard, you will focus on a subset of features, but netherless, you will build a fully functional server that can be used in low TRL (technology readiness levels) environments, for now.

## Development Style and Strategy

KISS, Keep it simple stupid. Don't over-engineer, don't over-design. Be straightforward.

## Working Strategy

You will work in an iterative way, and write documents to support yourself and other developers in your work, as you are doing it.

You have a `current_tasks` folder where you can write .txt or .md documents about the remaining tasks you have to do.

You also have a `ideas` folder, where you can also write down your ideas in `.txt` or `.md` files.

When a task or an idea is finished to a satisfactory level, you can move it to the `done` folder. You can check those folders if you are out of ideas of what to do, or if you want to check the progress of the project.

You can write design document, helping documentation, or any other kind of documentation in the doc folder.

You should commit to git once in a while, when it makes sense, like when some milestones are reached, or if it's significant changes. We are favouring small and frequent commits, over big and rare commits. Small commits give more insights.

## A JSON Only Server

FHIR supports JSON, XML, and Turtle formats.While the specification says "in the interests of interoperability, servers SHOULD support both the XML and JSON formats", we will only support JSON as XML is out of fashion. We can safely ignore Turtle and most of the RDF features as they aren't used by the industry in practice.

Here are some important notes about the JSON format from the FHIR specification:

- Comments are possible in XML but not possible in JSON (the format is not JSON with comments)
- For strings, leading and trailing whitespace is expected to be preserved in JSON. This difference is driven by XML Schema behavior
- Leading zeros are allowed for numbers in JSON but not XML
- FHIR is also used to exchange large amounts of data- 1000s of records, or more (up to billions). This specification documents ND-Json (New line delimited JSON) for this usage.

Source: https://build.fhir.org/resource-formats.html

## HTTP API

FHIR is more or less some fine RESTful API, defined there: https://build.fhir.org/http.html The API is comprehensive and not everything MUST or SHOULD be implemented.

## Testing

FHIR has some test suites, infortunately one is open-source and abandoned in a non-working state, and it looked pretty bare-bones. The two others are locked behind "contact us" gates, and they don't seem impressive either. The FHIR standards includes a TestScript resource, which can be used to define test cases on top of FHIR resources, but this is not including comprehensive test cases and the examples are minimal. Thus, this TestScript resource doesn't seem to be widely used in practice and it seem to re-invent the wheel of testing frameworks. It is present in FHIR 5 but not FHIR 6, apparently. The TestScript isn't part of the specification anymore and it has a new testing resource link that is 404 for now.

I think you will have to design your own test suite, to test comprehensively and well the features we want to support. Thankfully, most of the FHIR resources include good examples and documentation.

This is important to spend a lot of efforts and focus on writing extremely high-quality tests. It doesn't mean to write a huge quantity of them, but to write the right tests and moreover, to use them.

Sources:
- https://fhir.org/conformance-testing/
- https://hl7.org/fhir/testscript.html

## Norminative Content

Some features are"Norminative content", meaning it's part of a standard. While other features are in trial use or in development. It shouldn't matter much for our implementation.

## Validation and JSON Schema

An important part of the FHIR standard is making sure that the data coming in is valid.

Thankfully, FHIR provides an extensive JSON Schema for validation, which one can download. It's pretty big so do not attempt to read it in your context fully, but it is attached as `fhir.schema.json` for reference. It should probably be used in the server implementation to validate the incoming data.

https://docs.rs/jsonschema/latest/jsonschema/ seem like a good contender for this task.

As annoying as it sounds, we should always validate the schema, not an option.

## DataTypes

FHIR does a good job at defining data types and rules and validations. We should follow them. They als ohave some "expression", that one could probably interpret and reimplement in rust.

For example: https://build.fhir.org/datatypes.html

## FHIR reference repository

The FHIR reference repository is included in `fhir-reference-repository` folder. It includes *a lot* of examples and information, and use it and explore it wisely. In the source folder, each resource has its own folder with files named like `{type}-introduction.xml`, `{type}-notes.xml`, `{structuredefinition-Appointment.xml}`.

This documentation in XML is pretty verbose so you should likely use subagents to extract the relevant information, but you can also decide on your own what to do and how to handle it.

## Stack

As we stated, we are going to use Rust for the implementation. The HTTP server should use tower and tower-http. I included a small `rust-simple-project-reference` folder with another rust project that is going to use a similar HTTP stack. This project has some non relevant feature so don't consider it a reference for everything, but the tower, axum, utoipa, etc… that could be useful for you to get started I think.

### The Database: PostgreSQL

FHIR is apparently created around the concept of document stores, document database. Most implementations use MongoDB, with all the issues one could expect from it, like poor security, poor performance, and poor reliability. I personally implemented a FHIR server on top of CouchDB, a better database while incredibly slow, and while it worked, it wasn't a very successful project and the company eventually rewrote the FHIR server on top of PostGreSQL.

I think we could save a lot of time by just using PostgreSQL. It's the best.

I don't think we should go with an ORM. As good as Diesel is, sqlx is a better choice for us, allowing more flexibility and control. We should use sqlx.

Do not be tempted to use sqlite or any other database during development, we only use PostgreSQL.

We should use migrations when using sqlx.

### File attachments

FHIR supports file attachments, that can be relatively big. In the first version, we should keep it simple and just store them in postgresql. Eventually, one would expect postgresql to step up on the file storage, or we would have to connect to some S3 or similar, but for now we keep it simple.

### Schema tips

FHIR is big and complex and we have many ways to build a schema for it. The minimum is to use one table per resource type, with a JSONB column to store the data, some metadata columns, etc…

I don't think we should go full relational and enforce foreign key checks and so on, as FHIR seem to be designed around the concept of document stores. For now we could keep it simple, but, we can always use indexes on JSON fields to speed up some queries.

## Security

Security is important and we should make sure that our security model is safe and sound.

We should support multiple tenants, and JSON Web Tokens (JWT) for authentication and authorization. We would only validate the JSON Web Tokens, and extract the tenants from the claims. We won't implement the token issuing part, or the user management part.

We can also support a no-authenticated mode, for simplicity during testing and development, but it should be disabled by default.

We do not need advanced RBAC (Role Based Access Control) but some read-only, write-only, read-write permissions in the tokens could be useful. Also perhaps a possibility to restrict on the resource types only, with an allow-list. Nothing more fancy for now.

You can read more about security once in a while: https://build.fhir.org/security.html

## Capability Statement

The server should implement the Capability Statement resource well, as it's not going to support all features we should make sure that what is supported and working is well documented. Those capability statements are also actively used by many FHIR clients.

Source: https://build.fhir.org/capabilitystatement.html

## Dependencies Versions

Make sure to check that you use the latest versions of dependencies, using https://github.com/kbknapp/cargo-outdated

If some dependencies are too recent and not stable or incompatible with other dependencies, you are of course allowed to use slightly older versions, but the idea is to not stay stuck on very old versions.

## Final Words

You will work isolated and iterate by yourself. Be methodical, be profesional, and make sure to write good tests, to use them, to document your progress, your ideas, your tasks, and to maintain a good code base overall.

Thank you very much.