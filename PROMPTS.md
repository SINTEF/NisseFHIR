# Prompts

Development prompts and conversations used to build NisseFHIR, listed chronologically.

---

### Session 1 — GPT-5.3-Codex (autopilot)

> Good morning. I would like you to start working as stated in `fhir-specs.md`.
> Thank you very much.

---

### Session 2 — GPT-5.4 (autopilot)

> Can you please continue the good work on this project following `fhir-specs.md`?

Context provided — previous developer's summary:

> Implemented the first working milestone of the project and validated it end-to-end.
> Bootstrapped a Rust server crate with Axum + Tower HTTP + SQLx. Implemented startup/config,
> DB connection, migrations, JWT-based tenant extraction, scope/resource checks, health and
> CapabilityStatement endpoints, basic FHIR create/read/update routes, PostgreSQL JSONB storage,
> initial migration, project docs. Test suite: 5 passed, 0 failed. Full FHIR JSON Schema
> validation intentionally deferred to next slice.

---

### Session 3 — Claude Opus 4.6 (autopilot)

> Can you please continue the good work on this project following `fhir-specs.md`? I think we
> are getting there but the tests are severely lacking, while I insisted to be the core focus of
> the task… we have all the data to create excellent tests. Feel free to write conversion to
> generate the tests data. Right now it sounds like the previous developers discovered what are
> tests a few hours ago.

---

### Session 4 — Claude Opus 4.6 (autopilot)

> Please develop using `fhir-specs.md`.
>
> I notice that we are using many outdated dependencies, let's use `cargo outdated` first.
> Are we executing the tests in parallel? It looks a bit slow. Can you analyse the performance
> issue once you updated the dependencies?
>
> The database schema needs strong refinements. The current version is only one table, we should
> at least have some partitioning. I don't understand the index on the JSONB. The specs requested
> one table per data type, right? Perhaps not? Please continue the good work.

Follow-ups:

> Is PostgreSQL running sir?

> Are we using a PostgreSQL connection pool? Can we continue where we stopped?

---

### Session 5 — GPT-5.4 (autopilot)

> Can you continue the good work following `fhir-specs.md`?

---

### Session 6 — Claude Opus 4.6 (autopilot)

> Can you review what is done and what is missing according to `fhir-specs.md`? And then make a
> plan, write relevant documents, and keep working on the project until it's finished in a
> correct satisfactory state? Thank you.

Follow-up (interrupted):

> Sorry but yo! You don't use the explore agents correctly. Give them tasks to do in parallel.
> Asking them to return the file content obviously does NOT work. Use them with proper tasks, or
> don't use them.

---

### Session 7 — GPT-5.4 (autopilot)

> I included the examples folder, downloaded from `http://build.fhir.org/examples-json.zip`.
>
> I would like you to write a script, in Python this time, that is a real E2E test: it starts the
> server, either natively or through Docker, and then performs CRUD operations on the server using
> the examples folder. If the data isn't there it should download it automatically.
>
> Please test your script with both the native server and the Docker container, consider writing a
> docker-compose file to test the system.

Follow-ups:

> I don't know why you run the native mode on the dockerized PostgreSQL and not the local one sir.

> The Dockerfile uses a very outdated Rust version. Can you run the Python E2E on ALL the
> examples, perhaps with some parallelism to go faster?

> I think this is a good first version. Why are we rejecting the 3 examples? I assume some
> examples are actually invalid? But `json-edge-cases.json`, is it something we should support?
> Could we instead test them but expect a failure?
>
> Also we only test 160 files out of 2410 in the example folder. I think it's because you have a
> very restrictive way of selecting files and supported resource types. It should be better to
> parse the file and infer the types from file contents and not their filenames. We should support
> most types, perhaps all. For the ones we don't support, we should assert that we don't support
> them in the code. Can you improve the tests to do it like this? Thank you.

---

### Session 8 — Claude Opus 4.6 (autopilot)

> I found the repository `fhir-test-cases` (added as a submodule). It's a bit of a mess, so use
> it carefully, but some stuff may be useful. Can you run explore commands with specific tasks to
> find out what is useful and write documents about what could be done next?

Follow-ups:

> Good, let's continue and work on this methodically. Here are some thoughts:
>
> - I see a lack of utoipa and tower-helmet like in the `rusty-valkey-forward-auth` example.
>   Should we add them?
> - What is the request size limit? The E2E test fails on a 45 MB bundle, should we support it?
> - Would you support the `json-patch` feature? `json-patch = "4.1.0"`?
> - A natural next step is to make the server return a proper HTTP 413 plus OperationOutcome for
>   oversized payloads instead of letting that 45 MB bundle fail at the transport level.

> Stop using sub-agents to read files. Use them with proper interesting tasks.

---

### Session 9 — GPT-5.4 (autopilot)

> `fhir-specs.md`

---

### Session 10 — GPT-5.4 (autopilot)

> Can we implement the recommendations from `SECURITY_AUDIT.md` that make sense?
>
> - The default `"dev-secret-change-me"` is a terrible idea. Shouldn't we only verify the
>   signature of JWT? Why do we have a secret at all?
> - `CorsLayer::permissive` is a massive brain fart. Come on!
> - No rate limiting: better handled on other layers, not easy to implement well.
> - Audit logging: HTTP access logs like in `rusty-valkey-forward-auth` should be good enough.
> - No TLS: I usually let the ingress/proxy handle TLS.
> - `ALLOW_UNAUTHENTICATED`: remove this feature, completely erase it!
> - XSS in FHIR field: skill issue from the client side, we can ignore.

Follow-ups:

> Yes please do the next steps. I want to support good JWT logic, so either fetch JWT well or
> have a development default system but it has to be secure and not hardcoded MongoDB-like
> security.

> Remember to run/update the end-to-end tests in Python too ;)

---

### Session 11 — GPT-5.4 (autopilot)

> Could you review the SQL schema. Are we doing this correctly? Is it correct format? Are we
> following best practices?

Follow-up:

> 1. Yes, add the necessary checks and search indexes. Don't overdo it, focus on the minimum.
> 2. A history table sounds like a good idea. Let's implement this.
> 3. You can add more search indexes, but don't overdo it. Perhaps use Python to extract that
>    from the documentation XML.
>
> Also you don't need to migrate any data for now. We are at prototype scale still.

---

### Session 12 — Claude Opus 4.6 (autopilot)

> Using llvm-cov, I would like to work on improving the test coverage. Please run the tests with
> coverage instrumentation, check the reports, see what is not tested, write more tests, and
> repeat until we reach a satisfactory coverage level. Thank you.

---

### Session 13 — GPT-5.4 (autopilot)

> I noticed that Scalar (used for documentation) has telemetry enabled by default and uses CDN
> for assets. This is a big no-no. We cannot use it. We can try `utoipa-swagger-ui-vendored`
> instead.

Follow-ups:

> Nice, I think we should improve the documentation section to mention that one needs a valid
> token in the README, how to get one, and also have the option to configure tokens in the
> Swagger UI. Not sure how.

> I'm not sure I appreciate the `/dev/token` feature. This is not good practice. People will
> cut corners and just use it in production and never disable `JWT_MODE=dev`.
>
> Let's simplify:
> - Remove the dev mode for JWT, this is bad.
> - JWKS is fine, it should work with a Keycloak or similar.
> - Static/default is fine too.
>
> Perhaps one could use a Python script in the scripts folder to generate valid tokens in static
> mode, as long as they use the same `JWT_SECRET`?
>
> What's the tenant by default btw? Do we use `aud` or `iss`?

---

### Session 14 — GPT-5.4 (autopilot)

> We are validating the JSON schema, but are we validating more using
> `https://build.fhir.org/datatypes.html`? Is it part of the JSON schema? Can you write some
> unit tests to test the limits and see if we accept and refuse stuff, in a classic TDD approach,
> and then implement what is needed based on the results, and iterate?

Follow-ups:

> Yes I think you can continue to the next steps. For some of the validation, I assume you can
> find existing crates? If it's too difficult? But good job. Let's continue with a TDD approach.
> Write the tests first, they should fail, implement, see, iterate. Thank you.

> Do NOT use sub-agents to read files. Use sub-agents to perform meaningful exploration tasks
> such as finding patterns, summarization, identifying things.

---

### Session 15 — Claude Opus 4.6 (autopilot)

> Can you setup CI/CD with GitHub Actions? At the same time it would be neat if you add a Helm
> chart to deploy it in Kubernetes, with CloudNativePG support (assume it's already installed).
> You can look at `rusty-valkey-forward-auth` reference submodule for reference implementation.
> We want to release the charts too, so one needs to create an empty `gh-pages` branch, somehow.

---

### Session 16 — GPT-5.4 (autopilot)

> I'm not sure I like the pagination: it seems to be based on offset and count, which is
> beginner-level pagination. Can we instead use better pagination, i.e. the one that sorts and
> uses the `afterId` approach? It's slower but better IMHO.
>
> Is page size configurable? Do we have good tests on pagination? Can we have a good test where
> we generate quite a few documents, nothing dramatic, in which we can test pagination advanced
> scenarios?

Follow-up:

> I started PostgreSQL, and you can keep iterating. Make sure we get this perfect!

---

### Session 17 — Claude Opus 4.6 (autopilot)

> Hi, I think we should support the rules and search parameters: having more rules, more indexes
> in the database, in a good way.
>
> For example we look at Patient, are we using the patients correctly? I look at Location
> (`https://build.fhir.org/location.html#search`), I also see a lot of features regarding
> search, are we supporting them?
>
> I would really enjoy if you make a file that is the list of all the ResourceTypes. You can
> compute that list programmatically. You must iterate on the list resource type per resource type
> until all the search parameters and business rules of each resource type is correctly
> implemented. I do not want a simple Patient and let's move on thing. This should be a proper
> FHIR implementation that supports ALL official resource types. It's going to be a lot of code,
> so be methodical, organised, use files, memory, todo list, no shortcuts, take your time,
> iterate, run the tests, the linters, iterate again, don't get de-motivated. I count on you.

Follow-ups:

> Sorry the submodules weren't checked out, you can read them again if you wish.

> So it works, but do we have indexes for fast querying? Note that the indexes should handle when
> many documents have no value for specific fields and so on. Not everything can have indexes too
> I assume.

> You are the expert, but why so few indexes compared to the number of search parameters? Why
> only 17 resource types for the index? Are we providing this information in the capability
> statement document?
>
> Also Location looks cool: `https://build.fhir.org/location.html` — like "near" search that is
> very special. Perhaps we need some geospatial indexes?

> Yes.

---

### Session 18 — Claude Opus 4.6 (autopilot)

> I think the `ci.yml` is not using the new ARM nodes from GitHub, so it will be super slow. Can
> you look at how it's done in the `rusty-valkey-forward-auth` reference project? Can we also add
> pre-commit in GitHub and the pre-commit configuration file? pre-commit with gitleaks can be
> useful. You can add some `.gitlint` too.

---

### Session 19 — GPT-5.4 (autopilot)

> Can we update the CI for the chart release so that when we tag, we replace the chart version and
> the app version in `Chart.yaml` automatically? I always forget, it's annoying.

---

### Session 20 — GPT-5.3-Codex (autopilot)

> `fhir-specs.md`

Follow-up:

> Excellent! Did you make good tests related to the history feature? Can you use the code
> coverage tool (llvm-cov?) to make sure we are doing that well too?

---

### Session 21 — GPT-5.3-Codex (autopilot)

> Please continue as stated in `fhir-specs.md`.

Follow-ups:

> I understand, but isn't `If-Match` optional, or is it mandatory in the FHIR spec? I don't want
> a breaking change that is not compliant.

> Alright, make sure the tests still pass — the end-to-end, in Rust, in Python, everything.
> Let's go!

---

### Session 22 — GPT-5.3-Codex (autopilot)

> Please continue as stated in `fhir-specs.md`. Thanks.

---

### Session 23 — Claude Opus 4.6 (autopilot)

> Please continue as stated in `fhir-specs.md`. Thanks.

Follow-up:

> Sorry I panicked and interrupted. But please continue if you think this is worth supporting.
> I haven't supported such thing before because I thought it was a bit scary, but perhaps it
> makes sense to support. What do you think?

---

### Session 24 — Claude Opus 4.6 (autopilot)

> Please continue as stated in `fhir-specs.md`. Thanks.

---

### Session 25 — GPT-5.4 (autopilot)

> I think it's also time to tidy up the `current_tasks` and `ideas` folders, as most of them
> have been implemented by now. Can you check what can safely be removed? It's in the git history
> worst case.

Follow-ups:

> I would severely remove a lot of content of the files, or remove the ones that have been
> implemented and add the ideas or next steps as new ideas in the ideas folder.

> "Decide whether responses should move from `application/json` to `application/fhir+json`."
> Didn't we move to `application/fhir+json` already?

---

### Session 26 — GPT-5.4 (autopilot)

> I think your `Values.config.jwtSecret` not using a Secret is a bit of an issue in the Helm
> chart. I would use this to create a Secret, not hardcode it in `values.yaml`.
>
> Moreover, we should consider whether we use environment variables to set secrets, or whether we
> should read files instead. Perhaps we should support both. What do you think?

Follow-ups:

> Have you run the tests sir?

> I think you made the README a bit too complex/verbose now. This is good information, but
> perhaps keep that for the Helm charts folder README? That you could create now?

> Disabling JWT in the README is a bit too far from my taste. Keep it as simple as possible,
> perhaps add a feature to generate a random secret in the Helm chart if the secret is not
> specified, or use openssl like before also in the README quickstart.

---

### Session 27 — GPT-5.4 (autopilot)

> Can you make it so that if the `JWT_SECRET` is missing in static mode, we crash at launch.
>
> Also we shouldn't have a hardcoded secret in the `docker-compose.yml` file, even for local
> development. We should load it from an environment variable. We can update the README to
> reflect this change, and provide instructions on how to generate a secret for local development
> (openssl).
>
> The Python script to generate valid tokens can also be updated to use the environment variable
> for the secret.

---

### Session 28 — Claude Opus 4.6 (autopilot)

> I think we should re-organise the repository slightly: the `server/` folder content could most
> likely be at the root of the repository. I want to keep the current `README.md`, so
> `server/README.md` could be renamed to something else. I think the git submodules and other
> stuff could be moved to some "references" folder, and we need to update the docker-compose
> stuff, and other things such as the CI and so on. Sounds good?

Follow-ups:

> Stop using sub-agents to read files, that's wasteful.

> Please continue, the session was interrupted.

---

### Session 29 — Claude Opus 4.6 (autopilot)

> The CI fails, can you run the linter checks and perhaps the formatter and make sure everything
> works as it should?
>
> You can also run `pre-commit run --all-files` and configure the pre-commit to ignore false
> positives and fix what's needed too.

---

### Session 30 — GPT-5.4 (autopilot)

> I got an error while running the tests in GitHub. Can you confirm/check? Perhaps the tests are
> a bit sensitive to time?
>
> `conditional_create_multiple_matches_returns_412` failed: expected 412 for multiple matches,
> got 200.

Follow-up:

> Sorry PostgreSQL wasn't running. Run the test again takk.

---

### Session 31 — GPT-5.4 (autopilot)

> We got a weird bug in the `ci.yml` E2E tests: Docker reports containers as healthy, but the
> readiness wait loop times out. The server logs show `/fhir/metadata` returning 401.
>
> Is it some concurrency issue? Are we starting the server in the background correctly? Isn't
> this test completely unnecessary anyway? Are we expecting to get `/fhir/metadata` without a
> right token? Shouldn't we use the health endpoint?

---

### Session 32 — Claude Opus 4.6 (autopilot)

> Good work! We can now make the first release. Version 0.1.0!
>
> Can we have this release information in the source code, perhaps in the capabilities document?
> I think this document should include some information about the project. Also I think we can
> make a GitHub tag (`0.1.0` without a `v`), push it, and update the docker-compose file to use a
> pre-built Docker image with the right version. We can add a `CHANGELOG.md`. I may forget some
> important things about a release so please tell me and be proactive.

Follow-ups:

> Can we make version 0.1.1 now? Could you make a nice script that does a bit of everything
> easily, like updating the `Cargo.toml`, the `Cargo.lock`, create the commit, the tag, and so
> on. Keep it minimal. It could be in the scripts folder.

> Alright, we failed the checks, like `cargo check`. I think when we do a release we should also
> do some of the checks, at least the pre-commit stuff and the `cargo check` stuff like in GitHub
> CI. Running the tests would be a plus too. Can you improve the release script to do so, run it,
> see it fail, fix the failures (I think a `rustfmt` should do?), and then make a new 0.1.2
> release as 0.1.1 failed for some formatting issue.

---

### Session 33 — GPT-5.4 (autopilot)

> Doesn't look like we have resource type validation whatsoever? Like `/fhir/Canard` is the same
> as `/fhir/Patient` and `/fhir/ObviouslyNotAValidType`.
>
> Is this by design? If yes, tell me. If not, could we add a test that checks we reject invalid
> resource types, and then fix the code, in a good TDD approach. Thank you.

---

### Session 34 — GPT-5.4 (autopilot)

> Can you check the `SECURITY_AUDIT.md` file and see what is still relevant, clean up the old
> things.
>
> Moreover:
> - Rate limit shouldn't be done at this level.
> - Same for TLS/HTTPS, nice to have but well…
> - Stored XSS is a skill issue on the client side, beyond FHIR IMHO.
> - PUT creating resources "bypassing any future POST-specific validation" — is it really an
>   issue?
> - No optimistic concurrency control (ETag/If-Match) — do we support that now, right?
> - Resource version history — we should have it, right?
> - JWT secret strength not validated — beyond the scope.
> - Documentation exposed without authentication — it's a FHIR server sir.
> - Database credentials in compose file — fine for tests locally.
> - 50 MB is not excessive.
> - GET `/metadata` without auth — fine for me.

---

### Session 35 — GPT-5.4 (autopilot)

> We have a few references to `fhir-autopilot`, but now the project is named NisseFHIR, so fix
> the stuff, rename stuff, and so on — the release names, the container names in docker-compose,
> etc. Sorry to have missed that earlier…

---

### Session 36 — Claude Opus 4.6 (autopilot)

> Can you reformat the `PROMPTS.md` file so it looks neat? I think you can guess the current
> structure. It's a list of prompts and sessions. My current syntax is kind of a markdown mess
> where a lot of lines are considered to be titles. Make it tidy.
