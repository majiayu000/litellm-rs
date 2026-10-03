# Perplexity Sonar retirement audit

Issue #1373 / F10. Reviewed 2026-10-03; changes are unreleased.

The official [2026-08-13 announcement](https://community.perplexity.ai/t/sonar-is-moving-to-the-agent-api/5802)
sets September 27 as the Sonar endpoint retirement date. The official
[migration source at 1f1594c4](https://github.com/perplexityai/api-platform-developers/blob/1f1594c4250bb5cc97e69db78af3757cf2c8bc32/skills/migrate-sonar-to-agent-api/SKILL.md)
describes a change from `/chat/completions` to `/v1/agent` or `/v1/responses`, with
native input/output items, different tools and terminal events. Its older statement
that both APIs are live is superseded by the dated retirement announcement.
The live migration documentation URL was inaccessible to the research tool;
no authenticated provider call was made.

The runtime catalog previously declared ChatCompletion/ChatCompletionStream for
`perplexity`, sending every configured model through the retired chat endpoint.
The catalog entry is removed; canonical and alias selectors use the existing
UnsupportedEnum state and fail construction before network dispatch. The enum
identity and historical price rows remain, without promoting any pricing row
to callable. Generic explicit OpenAI-compatible configurations remain caller
configured endpoints; they are not advertised as Perplexity support.

The old uncompiled SDK auto-config source also no longer advertises this route.
Factory tests continue covering generic catalog behavior with AI21, and a new
regression verifies all three Perplexity selectors fail before transport.

Decision: remove the unsupported chat declaration now. Merely changing the base
URL would send the wrong request/response protocol. A future native Agent API or
verified embedding adapter is separate work; this change does not implement it
or claim that Perplexity's search/embedding products retired.
