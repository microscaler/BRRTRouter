# BFF Proxy — Wiki structure guide

**Purpose:** Define the [GitHub Wiki](https://github.com/microscaler/BRRTRouter/wiki) layout for BFF Proxy so all analysis, epics, and stories are available under a single **BFF_PROXY** section.

---

## MCP / API limitation

**The GitHub MCP used in this project does not expose wiki endpoints.** The user-github MCP provides:

- Repository contents: `get_file_contents`, `create_or_update_file`, `push_files` (main repo only)
- Issues and pull requests

GitHub’s wiki is a **separate git repository** (`BRRTRouter.wiki`). There is no REST API for creating or updating wiki pages; the wiki must be edited via the GitHub web UI or by cloning and pushing to the wiki repo.

So the BFF_PROXY wiki structure **cannot be created or updated via the current MCP**. Use one of the options below.

---

## Target wiki structure (all under BFF_PROXY)

GitHub wiki uses **page titles**; a “folder” is done with a slash in the title (e.g. `BFF-Proxy/Epics-and-Stories-Summary`). The sidebar will show a hierarchy when you use this naming.

| Wiki page title | Source in repo | Notes |
|-----------------|----------------|-------|
| **BFF-Proxy** | *(index — create manually or from README below)* | Landing page for BFF Proxy; link to Analysis, Summary, and Epics. |
| **BFF-Proxy/BFF-Proxy-Analysis** | `docs/BFF_PROXY_ANALYSIS.md` | Full analysis doc. |
| **BFF-Proxy/Epics-and-Stories-Summary** | `docs/EPICS/BFF_PROXY/EPICS_AND_STORIES_SUMMARY.md` | Summary + issue mapping. |
| **BFF-Proxy/Epic-1-Spec-Driven-Proxy** | `docs/EPICS/BFF_PROXY/epic-1-spec-driven-proxy/README.md` | Epic 1 overview (#254). |
| **BFF-Proxy/Epic-1-Story-1.1-Route-Meta-Extensions** | `docs/EPICS/BFF_PROXY/epic-1-spec-driven-proxy/story-1.1-route-meta-extensions.md` | |
| **BFF-Proxy/Epic-1-Story-1.2-BFF-Generator-Proxy-Extensions** | `docs/EPICS/BFF_PROXY/epic-1-spec-driven-proxy/story-1.2-bff-generator-proxy-extensions.md` | |
| **BFF-Proxy/Epic-1-Story-1.3-BFF-Generator-Components-Security** | `docs/EPICS/BFF_PROXY/epic-1-spec-driven-proxy/story-1.3-bff-generator-components-security.md` | |
| **BFF-Proxy/Epic-2-BFF-Proxy-Library** | `docs/EPICS/BFF_PROXY/epic-2-proxy-library/README.md` | Epic 2 overview (#255). |
| **BFF-Proxy/Epic-2-Story-2.1-Proxy-Library** | `docs/EPICS/BFF_PROXY/epic-2-proxy-library/story-2.1-proxy-library.md` | |
| **BFF-Proxy/Epic-2-Story-2.2-Downstream-Base-URL-Config** | `docs/EPICS/BFF_PROXY/epic-2-proxy-library/story-2.2-downstream-base-url-config.md` | |
| **BFF-Proxy/Epic-2-Story-2.3-Askama-Proxy-Handler** | `docs/EPICS/BFF_PROXY/epic-2-proxy-library/story-2.3-askama-proxy-handler.md` | |
| **BFF-Proxy/Epic-2-Story-2.4-BFF-Proxy-Integration** | `docs/EPICS/BFF_PROXY/epic-2-proxy-library/story-2.4-bff-proxy-integration.md` | |
| **BFF-Proxy/Epic-3-BFF-IDAM-Auth** | `docs/EPICS/BFF_PROXY/epic-3-bff-idam-auth/README.md` | Epic 3 overview (#256). |
| **BFF-Proxy/Epic-3-Story-3.1-BFF-OpenAPI-Security-Schemes** | `docs/EPICS/BFF_PROXY/epic-3-bff-idam-auth/story-3.1-bff-openapi-security-schemes.md` | |
| **BFF-Proxy/Epic-3-Story-3.2-Optional-Claims-Enrichment** | `docs/EPICS/BFF_PROXY/epic-3-bff-idam-auth/story-3.2-optional-claims-enrichment.md` | |
| **BFF-Proxy/Epic-3-Story-3.3-RBAC-From-JWT-Or-IDAM** | `docs/EPICS/BFF_PROXY/epic-3-bff-idam-auth/story-3.3-rbac-from-jwt-or-idam.md` | |
| **BFF-Proxy/Epic-4-Enrich-Downstream** | `docs/EPICS/BFF_PROXY/epic-4-enrich-downstream/README.md` | Epic 4 overview (#257). |
| **BFF-Proxy/Epic-4-Story-4.1-Proxy-Claim-Headers** | `docs/EPICS/BFF_PROXY/epic-4-enrich-downstream/story-4.1-proxy-claim-headers.md` | |
| **BFF-Proxy/Epic-4-Story-4.2-Configurable-Claim-Header-Mapping** | `docs/EPICS/BFF_PROXY/epic-4-enrich-downstream/story-4.2-configurable-claim-header-mapping.md` | |
| **BFF-Proxy/Epic-5-Microservices-Claims-Lifeguard** | `docs/EPICS/BFF_PROXY/epic-5-microservices-claims-lifeguard/README.md` | Epic 5 overview (#258). |
| **BFF-Proxy/Epic-5-Story-5.1-Expose-JWT-Claims-Typed-Handlers** | `docs/EPICS/BFF_PROXY/epic-5-microservices-claims-lifeguard/story-5.1-expose-jwt-claims-typed-handlers.md` | |
| **BFF-Proxy/Epic-5-Story-5.2-Lifeguard-Session-Claims** | `docs/EPICS/BFF_PROXY/epic-5-microservices-claims-lifeguard/story-5.2-lifeguard-session-claims.md` | |
| **BFF-Proxy/Epic-5-Story-5.3-Microservice-Auth-Model** | `docs/EPICS/BFF_PROXY/epic-5-microservices-claims-lifeguard/story-5.3-microservice-auth-model.md` | |

---

## How to create the wiki

### Option A — GitHub web UI

1. Go to [BRRTRouter Wiki](https://github.com/microscaler/BRRTRouter/wiki).
2. Create a new page with title **BFF-Proxy** (this will be the index).
3. For each row in the table above, create a page with the given **Wiki page title** and paste (or adapt) the content from the **Source in repo** file. Use the “BFF-Proxy/…” titles so they appear under the BFF-Proxy “folder” in the sidebar.

### Option B — Clone wiki repo and push

GitHub wikis are stored in a separate repo: `https://github.com/microscaler/BRRTRouter.wiki.git`.

1. Clone the wiki:  
   `git clone https://github.com/microscaler/BRRTRouter.wiki.git`
2. Wiki pages are Markdown files in the root; the filename becomes the page title (e.g. `BFF-Proxy.md`, `BFF-Proxy-Epics-and-Stories-Summary.md`). For a “folder” use e.g. `BFF-Proxy-Epics-and-Stories-Summary` (GitHub may show slashes in the UI from the *Home* link; check existing wiki file names).
3. Copy content from the repo paths above into the corresponding `.md` files in the wiki repo.
4. Commit and push to the wiki repo.

(Note: GitHub wiki file naming can use either spaces or hyphens; the exact mapping is documented in [GitHub’s wiki docs](https://docs.github.com/en/communities/documenting-your-project-with-wikis).)

### Option C — Sync script (future)

A small script could read the repo paths and create or update the wiki repo files so the wiki stays in sync with `docs/EPICS/BFF_PROXY/` and `docs/BFF_PROXY_ANALYSIS.md`. That would require push access to the wiki repo.

---

## Suggested BFF-Proxy index page (wiki landing)

Use this as the content for the **BFF-Proxy** wiki page (or adapt from `docs/EPICS/BFF_PROXY/README.md`):

```markdown
# BFF Proxy

Epics and stories for implementing BFF proxy behaviour in BRRTRouter (Phase 1: BFF ↔ IDAM ↔ Supabase, spec-driven proxy, Lifeguard with claims/row-based access; no LifeReflector).

## Contents

- [BFF Proxy Analysis](BFF-Proxy-Analysis) — Full analysis and recommendations
- [Epics and Stories Summary](Epics-and-Stories-Summary) — Summary and GitHub issue mapping

## Epics

1. [Epic 1: Spec-driven proxy (RouteMeta + BFF generator)](Epic-1-Spec-Driven-Proxy) — #254
2. [Epic 2: BFF proxy library and generated handlers](Epic-2-BFF-Proxy-Library) — #255
3. [Epic 3: BFF ↔ IDAM auth/RBAC](Epic-3-BFF-IDAM-Auth) — #256
4. [Epic 4: Enrich downstream with claims/RBAC](Epic-4-Enrich-Downstream) — #257
5. [Epic 5: Microservices claims + Lifeguard row-based access](Epic-5-Microservices-Claims-Lifeguard) — #258
```

(Adjust link syntax to match GitHub wiki: often `[Page-Name](Page-Name)` or `[Page Name](Page-Name)`.)

---

## References

- [GitHub Wiki](https://github.com/microscaler/BRRTRouter/wiki)
- [GitHub Docs: About wikis](https://docs.github.com/en/communities/documenting-your-project-with-wikis/about-wikis)
- Repo: `docs/BFF_PROXY_ANALYSIS.md`, `docs/EPICS/BFF_PROXY/`
