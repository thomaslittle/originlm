# ADR-004: client reuse without terminal dependencies; detection is not authorization

Date: 2026-10-28. Status: accepted; fixture qualification pending.

The facade, CLI status command and GPUI client reuse `originlm-control::BrokerClient`.
That package also builds a terminal application. Moving the shared code into a
new crate would create another distribution boundary without improving the
protocol. Keep the library, make terminal dependencies optional behind `tui`,
and disable default features in nonterminal consumers. The terminal binary
requires the feature. Workspace builds may unify it; isolated helper/facade
builds do not need ratatui or crossterm.

The client validates authentication and the protocol before accepting a broker,
bounds incoming frames and applies one deadline to the whole exchange. Credential
file readers reject links and broad Unix permissions; status also checks the
recorded endpoint before sending the token. Windows ACL qualification remains
separate from Unix mode checks.

Browser bridge detection is evidence of availability, never a grant. Opaque HTTP
responses cannot identify OriginLM, so detection reads and validates public
runtime info under the browser's normal same-origin/CORS rules. It only probes
loopback endpoints, omits credentials and refuses redirects. A badge says
“connected” only after its caller explicitly supplies authenticated status.
Page messages are restricted to the same window and origin, but a page ping is
still not a security boundary against scripts already running on that page.

The current HTTP host is same-origin only. Cross-origin website pairing and an
extension content-script bridge need separate reviewed implementations; they are
not implied by host-manifest support or the experimental detection package.
