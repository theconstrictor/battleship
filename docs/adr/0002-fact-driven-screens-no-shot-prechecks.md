# Client screens are fact-driven projections; shots are never pre-checked

The client's Screen transitions only in response to Facts or connection
events — never when it sends an Intent — and shot legality is never
computed locally: a bad shot costs one cheap `Fact::Error`. Fleet placement
is the single exception, where the client previews overlap/bounds and gates
Ready, because a rejected fleet is expensive to redo. The Authority
re-validates everything on receive in both cases.

**Considered options** — mirroring the server's phase machine client-side
(rejected: re-implements the authority on the render side) and symmetric
convenience checks for shots (rejected: risks rule drift for no real gain).
