# List of TODO for the human point of view

Can you make it so that if the JWT_SECRET is missing in static mode, we crash at lunch.

Also we shouldn't have a hardcoded secret in the docker-compose.yml file, even for local development. We should load it from an environment variable. We can update the README.md file to reflect this change, and provide instructions on how to generate a secret for local development (openssl).

The python script to generate valid tokens can also be updated to use the environment variable for the secret.

---
I think the ci.yml in the workflows is not using the new ARM nodes from github, so it will super slow. Can you look at how it's done in the rusty-valkey-forwar-auth reference project I included ? can we also take this time to add pre-commit in github and also the pre commit configuration file ?  pre-commit with git leaks can be useful. you can add some .gitlint too and so on.
