# Bevy client uses bevy_renet; net_client is a test harness only

We built `net_client` as a "reference client layer", but its design — a
background pump thread feeding an `mpsc` channel — fights Bevy's
single-world model (uncloneable channel, second clock, panic-on-death).
The Bevy client therefore uses `bevy_renet` directly, which holds the
`RenetClient` in a Resource and delivers wire bytes as Events on the same
`renet 2.0` stack. `net_client` remains the transport for the integration
tests and future headless clients (AI opponent); its crate docs say so.

**Considered options** — wrapping `net_client` in a Bevy Resource (rejected:
reimplements bevy_renet's bridge with two clocks) and growing `net_client`
into a Bevy-aware layer (rejected: diverges from Bevy idioms).
