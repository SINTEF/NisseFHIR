# List of TODO for the human point of view

Can you make it so that if the JWT_SECRET is missing in static mode, we crash at lunch.

Also we shouldn't have a hardcoded secret in the docker-compose.yml file, even for local development. We should load it from an environment variable. We can update the README.md file to reflect this change, and provide instructions on how to generate a secret for local development (openssl).

The python script to generate valid tokens can also be updated to use the environment variable for the secret.

---
The CI fails, can you run the linter checks and perhaps the formatter and make sure everything works as it should?

You can also run `pre-commit run  --all-files` and configure the pre-commit to ignore false positives and fix what's needed too.
