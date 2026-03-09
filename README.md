# NisseFHIR

NisseFHIR is a lightweight FHIR 6 server.

This server does not support all FHIR features, but it should be good enough to support most common use cases. If you need XML, RDF or some exotic FHIR features, this is not the right server.

⚠️ **This has been developed using AI agents. Use at your own risks.**

The human author states that NisseFHIR is a pretty good lightweight FHIR 6 server.

## Motivation

It comes from the need a good experimental FHIR server to develop advanced features for the [Invest4Health](https://invest4health.eu/) project. Funded by the European Union’s Horizon Europe Research and Innovation programme, under Grant Agreement 101095522.

Existing FHIR servers are [Not Invented Here](https://en.wikipedia.org/wiki/Not_invented_here) and using software stacks that are not appreciated by the author.

Software development has changed dramastically in the last years with the arrival of AI agents. It is now possible to develop complex software in a matter of hours, which can be tailored to very specific needs. This is a dramatic change to software engineering, as before one would almost never reinvent a wheel and rather spend time adapting to existing software.

While intimidating at first, a FHIR server is also not a huge piece of software if you restrict yourself to the most common and useful features.

## AI Agents cannot write good enough FHIR servers on their own

Reading the [Anthropic advertising experiment](https://www.anthropic.com/engineering/building-c-compiler) in which they built a C compiler using AI agents, I attempted to have AI agents build a FHIR 6 server. I wrote a comprehensive [specifications](./fhir-specs.md) document, attached the reference from FHIR, test data, and reference implementations using similar stacks.

In early March 2026, using a mix of GPT-5.4 and Claude Opus 4.6, this did not work. Not at all.

Perhaps bruteforcing the problem with infinite money could make it work eventually, but I reverted to the classic human expert in the loop, guiding AI agents.

The author did develop a FHIR 5 server in Golang for a healthcare company years ago, and had experience with the FHIR specifications. The server was used in production for years, and it eventually got replaced by something arguably better. Learning from this experience, it was easy to guide the AI agents, to not repeat the same mistakes but also quickly implement the important features.

You can read the [PROMPTS.md](./PROMPTS.md) file for the list of prompts used during the development.

## Software Stack and Architecture

NisseFHIR is written in Rust, and uses PostgreSQL as the database. It is stateless and can scale horizontally.

## Security

NisseFHIR supports JWT-based authentication and authorization. It follows best practices and uses a memory safe programming language. It is not perfect, but it should be good enough.

Importantly, the data is *not* encrypted at the application level. It is the responsability of the system administrator to configure encryption at rest and encryption in transit.

The author recommends setting up LUKS, and not relying on a checkbox conveniently provided by your favourite cloud provider. TLS encryption in transit could be done by the ingress, api gateway, reverse proxy, whatever you call it. Please note that content delivery network (CDN) providers decrypting and reencrypting the data should probably not be used.

## Testing and Validation

The server is heavily tested, as vibe-coded projects should be, using thousands of test cases from the HL7 FHIR resources. Moreover, it features a comprehensive end-to-end test suite, a high code coverage with unit and integration tests.

The data is also validated extensively using the official FHIR 6 JSON Schema.

## License

This project is open-source, dual licensed under the WTFPL and CeCILL-B licenses. See [LICENSE](./LICENSE) for details.

## Acknowledgements

This project has been developed for the [Invest4Health](https://invest4health.eu/) project, funded by the European Union’s Horizon Europe Research and Innovation programme, under Grant Agreement 101095522.