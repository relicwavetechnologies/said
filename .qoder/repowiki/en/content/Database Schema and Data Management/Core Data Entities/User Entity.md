# User Entity

<cite>
**Referenced Files in This Document**
- [users.rs](file://crates/backend/src/store/users.rs)
- [openai_oauth.rs](file://crates/backend/src/store/openai_oauth.rs)
- [openai_oauth_route.rs](file://crates/backend/src/routes/openai_oauth.rs)
- [cloud.rs](file://crates/backend/src/routes/cloud.rs)
- [prefs.rs](file://crates/backend/src/store/prefs.rs)
- [history.rs](file://crates/backend/src/store/history.rs)
- [vocabulary.rs](file://crates/backend/src/store/vocabulary.rs)
- [lib.rs](file://crates/backend/src/lib.rs)
- [main.rs](file://crates/backend/src/main.rs)
- [001_initial.sql](file://crates/backend/src/store/migrations/001_initial.sql)
- [006_openai_oauth.sql](file://crates/backend/src/store/migrations/006_openai_oauth.sql)
- [auth.rs](file://crates/control-plane/src/routes/auth.rs)
- [store.rs](file://crates/control-plane/src/store.rs)
</cite>

## Table of Contents
1. [Introduction](#introduction)
2. [Project Structure](#project-structure)
3. [Core Components](#core-components)
4. [Architecture Overview](#architecture-overview)
5. [Detailed Component Analysis](#detailed-component-analysis)
6. [Dependency Analysis](#dependency-analysis)
7. [Performance Considerations](#performance-considerations)
8. [Troubleshooting Guide](#troubleshooting-guide)
9. [Conclusion](#conclusion)
10. [Appendices](#appendices)

## Introduction
This document describes the User entity and its lifecycle in WISPR Hindi Bridge. It covers the user record structure, license tier management, account creation timestamps, and user profile data. It also documents user account lifecycle operations including registration, authentication integration with OpenAI OAuth, license management, and account deactivation. The relationships between users and their data (recordings, preferences, vocabulary) are explained, along with multi-user support architecture and user data isolation. Examples of user account operations, cloud synchronization features, and user preference inheritance patterns are included. Privacy, security, and compliance considerations are addressed.

## Project Structure
The user domain spans two primary areas:
- Backend local daemon (SQLite): stores user identity, preferences, history, vocabulary, and OpenAI OAuth tokens.
- Control plane (PostgreSQL): manages cloud accounts, sessions, and license tiers.

```mermaid
graph TB
subgraph "Desktop App"
UI["Settings / Dashboard Views"]
end
subgraph "Backend Daemon"
MAIN["main.rs<br/>Startup, default user, timers"]
LIB["lib.rs<br/>AppState, router, caches"]
USERS["store/users.rs<br/>LocalUser, cloud token, license tier"]
PREFS["store/prefs.rs<br/>Preferences, API keys, LLM provider"]
HISTORY["store/history.rs<br/>Recordings"]
VOCAB["store/vocabulary.rs<br/>Vocabulary terms"]
OAUTH_STORE["store/openai_oauth.rs<br/>OpenAI tokens"]
OAUTH_ROUTE["routes/openai_oauth.rs<br/>OAuth init/status/disconnect"]
CLOUD_ROUTE["routes/cloud.rs<br/>Cloud token bridge"]
end
subgraph "Control Plane"
CP_STORE["control-plane/store.rs<br/>PostgreSQL schema"]
CP_AUTH["control-plane/routes/auth.rs<br/>Signup/Login/Logout/Me"]
end
UI --> LIB
LIB --> USERS
LIB --> PREFS
LIB --> HISTORY
LIB --> VOCAB
LIB --> OAUTH_ROUTE
LIB --> CLOUD_ROUTE
OAUTH_ROUTE --> OAUTH_STORE
MAIN --> USERS
MAIN --> PREFS
CP_AUTH --> CP_STORE
```

**Diagram sources**
- [main.rs:56-78](file://crates/backend/src/main.rs#L56-L78)
- [lib.rs:150-199](file://crates/backend/src/lib.rs#L150-L199)
- [users.rs:6-13](file://crates/backend/src/store/users.rs#L6-L13)
- [prefs.rs:6-25](file://crates/backend/src/store/prefs.rs#L6-L25)
- [history.rs:7-26](file://crates/backend/src/store/history.rs#L7-L26)
- [vocabulary.rs:22-29](file://crates/backend/src/store/vocabulary.rs#L22-L29)
- [openai_oauth.rs:7-14](file://crates/backend/src/store/openai_oauth.rs#L7-L14)
- [openai_oauth_route.rs:116-201](file://crates/backend/src/routes/openai_oauth.rs#L116-L201)
- [cloud.rs:20-60](file://crates/backend/src/routes/cloud.rs#L20-L60)
- [store.rs:17-34](file://crates/control-plane/src/store.rs#L17-L34)
- [auth.rs:46-159](file://crates/control-plane/src/routes/auth.rs#L46-L159)

**Section sources**
- [main.rs:56-78](file://crates/backend/src/main.rs#L56-L78)
- [lib.rs:150-199](file://crates/backend/src/lib.rs#L150-L199)

## Core Components
- LocalUser: identity and cloud linkage for the single default user.
- Preferences: per-user settings, API keys, and LLM routing.
- Recordings: user’s speech-to-text polish history.
- Vocabulary: STT bias terms keyed by user.
- OpenAI OAuth: local token storage and provider switching.
- Cloud bridge: store/clear cloud token and license tier locally.
- Control plane auth: cloud account management and license tiers.

**Section sources**
- [users.rs:6-13](file://crates/backend/src/store/users.rs#L6-L13)
- [prefs.rs:6-25](file://crates/backend/src/store/prefs.rs#L6-L25)
- [history.rs:7-26](file://crates/backend/src/store/history.rs#L7-L26)
- [vocabulary.rs:22-29](file://crates/backend/src/store/vocabulary.rs#L22-L29)
- [openai_oauth.rs:7-14](file://crates/backend/src/store/openai_oauth.rs#L7-L14)
- [cloud.rs:20-60](file://crates/backend/src/routes/cloud.rs#L20-L60)
- [auth.rs:46-159](file://crates/control-plane/src/routes/auth.rs#L46-L159)

## Architecture Overview
WISPR Hindi Bridge runs a local backend daemon with a default single user. The desktop app communicates with the daemon via a shared-secret bearer token. Cloud account management and licensing live in the control plane. Users can connect an OpenAI account for enhanced LLM capabilities, and optionally synchronize usage metrics to the cloud.

```mermaid
sequenceDiagram
participant UI as "Desktop App"
participant BE as "Backend Daemon"
participant DB as "SQLite (local_user, prefs, history, vocab, oauth)"
participant CP as "Control Plane (PostgreSQL)"
UI->>BE : "PUT /v1/cloud/token {token, license_tier}"
BE->>DB : "UPDATE local_user.cloud_token, license_tier"
BE-->>UI : "204 No Content"
UI->>BE : "GET /v1/cloud/status"
BE->>DB : "SELECT local_user.*"
BE-->>UI : "{connected, license_tier, email}"
UI->>BE : "POST /v1/openai-oauth/initiate"
BE-->>UI : "{auth_url}"
UI->>BE : "DELETE /v1/openai-oauth/disconnect"
BE->>DB : "DELETE openai_oauth, UPDATE preferences.llm_provider"
BE-->>UI : "204 No Content"
BE->>CP : "Hourly metering report (if cloud_token present)"
```

**Diagram sources**
- [cloud.rs:28-60](file://crates/backend/src/routes/cloud.rs#L28-L60)
- [openai_oauth_route.rs:116-201](file://crates/backend/src/routes/openai_oauth.rs#L116-L201)
- [openai_oauth.rs:36-83](file://crates/backend/src/store/openai_oauth.rs#L36-L83)
- [main.rs:151-233](file://crates/backend/src/main.rs#L151-L233)

## Detailed Component Analysis

### User Record Structure
- Identity: unique id, email, license tier, created_at.
- Cloud linkage: optional cloud_token for cloud synchronization and metering.
- License tier: managed by the control plane; reflected locally for UI and feature gating.

```mermaid
erDiagram
LOCAL_USER {
text id PK
text email
text cloud_token
text license_tier
int created_at
}
PREFERENCES {
text user_id FK
text selected_model
text tone_preset
text custom_prompt
text language
text output_language
boolean auto_paste
boolean edit_capture
text polish_text_hotkey
int updated_at
text gateway_api_key
text deepgram_api_key
text gemini_api_key
text groq_api_key
text llm_provider
}
RECORDINGS {
text id PK
text user_id FK
int timestamp_ms
text transcript
text polished
text final_text
int word_count
float recording_seconds
text model_used
float confidence
int transcribe_ms
int embed_ms
int polish_ms
text target_app
int edit_count
text source
text audio_id
}
VOCABULARY {
text user_id FK
text term
float weight
int use_count
int last_used
text source
}
OPENAI_OAUTH {
text user_id PK FK
text access_token
text refresh_token
int expires_at
int connected_at
}
LOCAL_USER ||--o| PREFERENCES : "1:1"
LOCAL_USER ||--o{ RECORDINGS : "1:N"
LOCAL_USER ||--o{ VOCABULARY : "1:N"
LOCAL_USER ||--o| OPENAI_OAUTH : "1:1"
```

**Diagram sources**
- [001_initial.sql:8-48](file://crates/backend/src/store/migrations/001_initial.sql#L8-L48)
- [006_openai_oauth.sql:4-10](file://crates/backend/src/store/migrations/006_openai_oauth.sql#L4-L10)
- [users.rs:6-13](file://crates/backend/src/store/users.rs#L6-L13)
- [prefs.rs:6-25](file://crates/backend/src/store/prefs.rs#L6-L25)
- [history.rs:7-26](file://crates/backend/src/store/history.rs#L7-L26)
- [vocabulary.rs:22-29](file://crates/backend/src/store/vocabulary.rs#L22-L29)
- [openai_oauth.rs:7-14](file://crates/backend/src/store/openai_oauth.rs#L7-L14)

**Section sources**
- [users.rs:6-13](file://crates/backend/src/store/users.rs#L6-L13)
- [001_initial.sql:8-48](file://crates/backend/src/store/migrations/001_initial.sql#L8-L48)
- [006_openai_oauth.sql:4-10](file://crates/backend/src/store/migrations/006_openai_oauth.sql#L4-L10)

### User Account Lifecycle Operations

#### Registration and Authentication Integration
- Cloud account registration and login occur in the control plane. The backend does not manage cloud accounts directly.
- After login, the desktop app stores a cloud bearer token and license tier locally via the backend’s cloud bridge endpoints.

```mermaid
sequenceDiagram
participant UI as "Desktop App"
participant CP as "Control Plane"
participant BE as "Backend Daemon"
participant DB as "SQLite"
UI->>CP : "POST /v1/auth/signup or login"
CP-->>UI : "{token, account{id,email,license_tier}}"
UI->>BE : "PUT /v1/cloud/token {token, license_tier}"
BE->>DB : "UPDATE local_user SET cloud_token, license_tier"
BE-->>UI : "204 No Content"
UI->>BE : "GET /v1/cloud/status"
BE->>DB : "SELECT local_user"
BE-->>UI : "{connected, license_tier, email}"
```

**Diagram sources**
- [auth.rs:46-159](file://crates/control-plane/src/routes/auth.rs#L46-L159)
- [cloud.rs:28-60](file://crates/backend/src/routes/cloud.rs#L28-L60)
- [users.rs:15-31](file://crates/backend/src/store/users.rs#L15-L31)

**Section sources**
- [auth.rs:46-159](file://crates/control-plane/src/routes/auth.rs#L46-L159)
- [cloud.rs:28-60](file://crates/backend/src/routes/cloud.rs#L28-L60)
- [users.rs:15-31](file://crates/backend/src/store/users.rs#L15-L31)

#### License Management
- License tier originates from the control plane and is mirrored locally.
- The backend enforces feature differences via license tier and preferences (e.g., history retention, model availability).
- The control plane defines feature sets per tier.

**Section sources**
- [auth.rs:233-248](file://crates/control-plane/src/routes/auth.rs#L233-L248)
- [users.rs:6-13](file://crates/backend/src/store/users.rs#L6-L13)

#### Account Deactivation and Token Clearing
- Clearing the cloud token locally removes cloud linkage and resets license tier to “free”.
- Disconnecting OpenAI OAuth removes tokens and reverts LLM provider to the default gateway.

```mermaid
sequenceDiagram
participant UI as "Desktop App"
participant BE as "Backend Daemon"
participant DB as "SQLite"
UI->>BE : "DELETE /v1/cloud/token"
BE->>DB : "UPDATE local_user SET cloud_token=NULL, license_tier='free'"
BE-->>UI : "204 No Content"
UI->>BE : "DELETE /v1/openai-oauth/disconnect"
BE->>DB : "DELETE openai_oauth"
BE->>DB : "UPDATE preferences SET llm_provider='gateway'"
BE-->>UI : "204 No Content"
```

**Diagram sources**
- [cloud.rs:43-46](file://crates/backend/src/routes/cloud.rs#L43-L46)
- [users.rs:24-31](file://crates/backend/src/store/users.rs#L24-L31)
- [openai_oauth_route.rs:195-201](file://crates/backend/src/routes/openai_oauth.rs#L195-L201)
- [openai_oauth.rs:70-83](file://crates/backend/src/store/openai_oauth.rs#L70-L83)

**Section sources**
- [cloud.rs:43-46](file://crates/backend/src/routes/cloud.rs#L43-L46)
- [users.rs:24-31](file://crates/backend/src/store/users.rs#L24-L31)
- [openai_oauth_route.rs:195-201](file://crates/backend/src/routes/openai_oauth.rs#L195-L201)
- [openai_oauth.rs:70-83](file://crates/backend/src/store/openai_oauth.rs#L70-L83)

### Multi-User Support and Data Isolation
- Single default user is created automatically at first run and identified by a fixed default user id.
- All local tables reference the default user id, ensuring strict per-user isolation.
- There is no multi-user branching in the backend; all data belongs to the default user.

```mermaid
flowchart TD
Start(["Startup"]) --> EnsureUser["Ensure default user exists"]
EnsureUser --> HasUser{"User exists?"}
HasUser --> |Yes| UseExisting["Use existing default user id"]
HasUser --> |No| CreateUser["Create default user + default prefs"]
UseExisting --> Ready["Ready"]
CreateUser --> Ready
```

**Diagram sources**
- [main.rs:58-78](file://crates/backend/src/main.rs#L58-L78)
- [store_mod.rs:182-215](file://crates/backend/src/store/mod.rs#L182-L215)

**Section sources**
- [main.rs:58-78](file://crates/backend/src/main.rs#L58-L78)
- [store_mod.rs:182-215](file://crates/backend/src/store/mod.rs#L182-L215)

### User Data Relationships and Inheritance Patterns
- Preferences are cached in memory to avoid frequent SQLite reads and are invalidated on updates.
- Lexicon cache combines corrections and STT replacements to reduce synchronous reads.
- Vocabulary terms influence STT behavior and are user-scoped.

```mermaid
classDiagram
class CachedPrefs {
+prefs : Preferences
+cached_at : Instant
}
class PrefsCache {
+get(user_id) Preferences
+invalidate()
}
class CachedLexicon {
+corrections : Vec<Correction>
+stt_replacements : Vec<SttReplacement>
+cached_at : Instant
}
class LexiconCache {
+get(user_id) (Corrections, SttReplacements)
+invalidate()
}
PrefsCache --> CachedPrefs : "holds"
LexiconCache --> CachedLexicon : "holds"
```

**Diagram sources**
- [lib.rs:31-69](file://crates/backend/src/lib.rs#L31-L69)
- [lib.rs:79-131](file://crates/backend/src/lib.rs#L79-L131)

**Section sources**
- [lib.rs:31-69](file://crates/backend/src/lib.rs#L31-L69)
- [lib.rs:79-131](file://crates/backend/src/lib.rs#L79-L131)

### Cloud Synchronization and Metering
- The backend can send daily usage metrics to the cloud when a cloud token is present.
- The cloud token and license tier are stored locally for UI and feature decisions.

```mermaid
sequenceDiagram
participant BE as "Backend Daemon"
participant DB as "SQLite"
participant CP as "Cloud API"
BE->>DB : "SELECT local_user.cloud_token"
alt token present
BE->>DB : "Aggregate recordings (last 7 days)"
BE->>CP : "POST /v1/metering/report (Bearer cloud_token)"
CP-->>BE : "2xx OK"
else no token
BE-->>BE : "Skip reporting"
end
```

**Diagram sources**
- [main.rs:151-233](file://crates/backend/src/main.rs#L151-L233)
- [users.rs:33-50](file://crates/backend/src/store/users.rs#L33-L50)

**Section sources**
- [main.rs:151-233](file://crates/backend/src/main.rs#L151-L233)
- [users.rs:33-50](file://crates/backend/src/store/users.rs#L33-L50)

### OpenAI OAuth Integration
- The backend initiates OAuth with PKCE, validates state, exchanges code for tokens, and persists them locally.
- On successful connection, the LLM provider preference switches to OpenAI Codex; disconnecting reverts to the default gateway.

```mermaid
sequenceDiagram
participant UI as "Desktop App"
participant BE as "Backend Daemon"
participant OA as "OpenAI OAuth"
participant DB as "SQLite"
UI->>BE : "POST /v1/openai-oauth/initiate"
BE-->>UI : "{auth_url}"
UI->>OA : "Authorize and redirect to localhost : 1455"
BE->>BE : "Validate state, PKCE"
BE->>OA : "Exchange code for tokens"
BE->>DB : "INSERT openai_oauth"
BE->>DB : "UPDATE preferences.llm_provider='openai_codex'"
UI->>BE : "GET /v1/openai-oauth/status"
BE-->>UI : "{connected, expires_at, models}"
UI->>BE : "DELETE /v1/openai-oauth/disconnect"
BE->>DB : "DELETE openai_oauth, UPDATE llm_provider='gateway'"
```

**Diagram sources**
- [openai_oauth_route.rs:116-201](file://crates/backend/src/routes/openai_oauth.rs#L116-L201)
- [openai_oauth_route.rs:205-308](file://crates/backend/src/routes/openai_oauth.rs#L205-L308)
- [openai_oauth.rs:36-83](file://crates/backend/src/store/openai_oauth.rs#L36-L83)

**Section sources**
- [openai_oauth_route.rs:116-201](file://crates/backend/src/routes/openai_oauth.rs#L116-L201)
- [openai_oauth_route.rs:205-308](file://crates/backend/src/routes/openai_oauth.rs#L205-L308)
- [openai_oauth.rs:36-83](file://crates/backend/src/store/openai_oauth.rs#L36-L83)

## Dependency Analysis
- Backend depends on SQLite for local persistence and on the control plane for cloud account management.
- The desktop app acts as the orchestrator for user actions against the backend and control plane.
- The backend maintains internal caches for preferences and lexicon to optimize performance.

```mermaid
graph LR
UI["Desktop App"] --> BE["Backend Daemon"]
BE --> SQLITE["SQLite (local_user, prefs, history, vocab, oauth)"]
BE --> CP["Control Plane (auth, license)"]
BE --> OA["OpenAI OAuth"]
```

**Diagram sources**
- [lib.rs:150-199](file://crates/backend/src/lib.rs#L150-L199)
- [main.rs:56-78](file://crates/backend/src/main.rs#L56-L78)
- [auth.rs:46-159](file://crates/control-plane/src/routes/auth.rs#L46-L159)

**Section sources**
- [lib.rs:150-199](file://crates/backend/src/lib.rs#L150-L199)
- [main.rs:56-78](file://crates/backend/src/main.rs#L56-L78)
- [auth.rs:46-159](file://crates/control-plane/src/routes/auth.rs#L46-L159)

## Performance Considerations
- Preference and lexicon caching reduces SQLite overhead for frequently accessed data.
- Background tasks handle periodic cleanup and metering to keep the daemon responsive.
- API keys and tokens are stored locally and never leave the device.

[No sources needed since this section provides general guidance]

## Troubleshooting Guide
- Unauthorized access: ensure the desktop app sends the shared-secret bearer token with every request.
- OAuth failures: verify PKCE state matches, network connectivity to OpenAI, and that the callback server binds successfully.
- Cloud token issues: confirm the token is stored locally and not expired; check metering reports for errors.
- Data isolation: confirm all tables reference the default user id and that no cross-user queries are attempted.

**Section sources**
- [auth/mod.rs:19-37](file://crates/backend/src/auth/mod.rs#L19-L37)
- [openai_oauth_route.rs:261-276](file://crates/backend/src/routes/openai_oauth.rs#L261-L276)
- [main.rs:151-233](file://crates/backend/src/main.rs#L151-L233)

## Conclusion
The User entity in WISPR Hindi Bridge centers on a single default user with robust local persistence and controlled cloud integration. Identity, preferences, history, vocabulary, and OAuth tokens are tightly scoped to the default user, ensuring strong data isolation. The control plane manages cloud accounts and licenses, while the backend provides secure, cached access to user data and integrates with OpenAI OAuth and cloud metering.

[No sources needed since this section summarizes without analyzing specific files]

## Appendices

### User Data Privacy, Security, and Compliance
- Tokens and API keys are stored locally and never leave the device.
- Metering reports are sent only when a cloud token is present and are protected by bearer authentication.
- OAuth uses PKCE and state validation to mitigate interception risks.
- License tiers and feature gating are enforced locally based on the stored license tier.

**Section sources**
- [openai_oauth_route.rs:30-48](file://crates/backend/src/routes/openai_oauth.rs#L30-L48)
- [openai_oauth.rs:36-83](file://crates/backend/src/store/openai_oauth.rs#L36-L83)
- [main.rs:151-233](file://crates/backend/src/main.rs#L151-L233)