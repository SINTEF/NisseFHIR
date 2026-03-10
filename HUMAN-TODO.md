# List of TODO for the human point of view
The CI fails, can you run the linter checks and perhaps the formatter and make sure everything works as it should?

You can also run `pre-commit run  --all-files` and configure the pre-commit to ignore false positives and fix what's needed too.
---
I think we should re-organise the repository slightly: the `server/` folder content could most likely be at at the root of the repository. I want to keep the current README.md, so server/README.md could be renamed to something else, and perhaps slightly improved/trimmed down at the same time. I think the git submodules and other stuff could be moved to some "references" folder, and we need to update the docker compose stuff, and other things such as the CI and so on. sounds good ?
