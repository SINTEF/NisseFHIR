# Patient compartment coverage

The executable R6 search registry is the machine-readable source for the
first `$everything` release. It currently maps **62 resource types** to a
direct Reference-valued `patient` search parameter. A unit test pins that
count and representative clinical types so regeneration cannot silently
reduce coverage.

- Direct: all 62 registry types with an executable `patient` Reference path.
- Derived: none. Reverse membership is never inferred by graph traversal.
- Unsupported: compartment relationships which require chaining, cohort
  evaluation, or a non-Reference expression. They remain deliberately
  unadvertised until represented by explicit, tested joins.

Supporting resources are not compartment members. They are selected by the
separate bounded allow-list in `src/everything/mod.rs` (depth one generally;
depth two only through `PractitionerRole`).
