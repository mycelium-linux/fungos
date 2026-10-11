# Local intranet integration

This is an optional integration slice, not a new base-image daemon or a claim
that LAN DNS is deployed. Keep the independent owners:

| Owner | Responsibility |
|---|---|
| Mycelium | Mesh/site names, topology, reachability, access and update policy; resolver configuration/delegation planning |
| Shroud | Stable workload service identity, named ports, serving revisions and replicas; its own DNS namespace |
| Palisade | Stable HTTP ingress, upstream routing, TLS and application access policy |
| Unibus | Application transport and consumption of authorized service discovery |
| Site DNS implementation | Serving records and conditional forwarding through its owning configuration driver |

## Names and transport

Ordinary browsers use an approved HTTPS name resolving to stable ingress.
Replacing a workload must not require browsers to learn revision IPs. Shroud's
`.shroud.svc` members remain Shroud-owned; Mycelium delegates that namespace
instead of copying replicas into its inventory-derived DNS records.

The existing Shroud audit tracks prepared-name versus serving-alias semantics
separately. Do not infer healthy promoted replicas from a resolvable name or
claim that Beachhead's resolver/serving-alias rollout has been completed here.

Mycelium's read-only `dns plan --site SITE --delegations FILE` produces site
records and conditional forwarding intent. It neither changes default DNS nor
installs a resolver. See the owning Mycelium `docs/site-dns.md` runbook for format,
validation and activation gates. The optional Palisade intranet names endpoint
is bearer-protected and reports explicit frontend intent separately from accepted
backend route membership; see Palisade's README. Both implementations are local
development work, not yet shipped in the fungOS image or installed CLI.

Unibus continues to consume provider-neutral application service observations.
Iroh endpoint IDs and authenticated bindings stay transport metadata; an endpoint
ID is not an IP address, and DNS does not confer an application grant. Browser
access, Shroud and Palisade do not require Unibus.

## First qualification

Use an isolated guest/network and one named service, not the production resolver.
Keep DHCP/default uplink configuration unchanged. Verify local peer records,
delegated positive and negative answers, unrelated public DNS and normal HTTPS
from a client. Replace the workload and confirm the name stays stable while
Shroud's serving endpoints change. Restart the resolver/runtime and repeat.

Mycelium resolver activation still needs a persistent apply/rollback/read-only
verification driver. Until that exists, a generated fragment is a plan, not an
applied change. Do not bypass the missing contract with ad hoc router commands.
Use the existing shared-infra Shroud split-DNS attachment for its bridge namespace;
do not transfer bridge ownership to Mycelium or mask systemd-resolved.

The existing cloud/headless-edge image declarations remain unchanged. A future
optional intranet package/profile can compose these components after owner
packages and whole-path qualification are available; no second registry or
unconditional DNS server belongs in the base image.
