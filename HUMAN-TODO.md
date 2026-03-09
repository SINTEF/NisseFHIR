# List of TODO for the human point of view

- I see a lack of utoipa and tower-helmet like in the rusty-valkey-forward-auth example project, do you think we should add them? yes or no?
- What is the request size limit? I think the e2e test fail on a 45MB bundle, should we support it?
- Would you support the json-patch feature? json-patch = "4.1.0" ?


You are a white hat hacker. Your task is to hack this FHIR server, you will try the best tools and strategy to break it, and make a neat report of what you tried, what worked, and what didn't, and suggestions for the dev team about what to do next. This is a low TRL prototype so expect to find a lot of stuff. You can read the code.

A natural next step is to make the server return a proper HTTP 413 plus OperationOutcome for oversized payloads instead of letting that 45 MB bundle fail at the transport level.Expanded the E2E harness to scan all example files by parsed resourceType, classify accepted/invalid/unsupported/transport-limited outcomes, verified the full 2410-file run in both native and Docker modes, and updated the Docker builder image and supporting docs to match the broader coverage.