# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Glyphkeel contributors
#
# The MCP server's own messages, in English. Agents read them as they are;
# errors carry the message's ID and arguments so the app can show its own
# translation. Tool names, field names and the labels of an error's details
# are machine text and stay English. One message per line; placeables are
# { $name }.

## Tools

mcp-no-such-tool = There is no tool "{ $tool }".
mcp-no-such-tool-hint = Use one of the listed tools.
mcp-tool-not-allowed = Your profile ({ $profile }) doesn't allow { $tool }, which makes { $safety } changes.
mcp-tool-not-allowed-hint = Ask the person to allow more in Preferences › Agents, or to start the server with a higher --profile.
mcp-rate-limit = { $max ->
        [one] This session made one change in the last minute, the most allowed.
       *[other] This session made { $max } changes in the last minute, the most allowed.
    }
mcp-rate-limit-hint = { $seconds ->
        [one] Wait one second before the next change.
       *[other] Wait { $seconds } s before the next change.
    }

## Starting the server

mcp-no-data-folder = No data folder found: HOME is not set.
mcp-no-data-folder-hint = Set HOME or XDG_DATA_HOME, or pass --workspace with a path.
mcp-unknown-toolset = unknown toolset "{ $toolset }"; use { $toolsets }
mcp-unknown-log-level = unknown log level "{ $level }"; use off, error, warn, info, debug or trace

## Prompts

mcp-no-such-prompt = There is no prompt "{ $prompt }".
mcp-no-such-prompt-hint = Use one of the listed prompts.
mcp-prompt-not-allowed = Your profile ({ $profile }) doesn't allow the prompt { $prompt }, or its tools are switched off: it uses tools that aren't listed for you.
mcp-prompt-not-allowed-hint = Ask the person to allow more in Preferences › Agents, or to start the server with a higher --profile or more --toolsets.
mcp-prompt-missing-argument = The prompt { $prompt } needs the argument "{ $argument }".
mcp-prompt-unknown-argument = The prompt { $prompt } has no argument "{ $argument }".

## Confirmations: the person sees these in the agent's client

mcp-confirm-beyond-profile = { $agent } wants to use “{ $title }”, which makes { $safety } changes that its profile ({ $profile }) doesn't allow. Allow it this once?
mcp-confirm-delete-project = { $tasks ->
        [0] { $agent } wants to delete the project { $project } for good. Allow?
        [one] { $agent } wants to delete the project { $project } with one task for good. Allow?
       *[other] { $agent } wants to delete the project { $project } with { $tasks } tasks for good. Allow?
    }
mcp-confirm-purge-trash = { $count ->
        [one] { $agent } wants to delete one item from the trash for good. Allow?
       *[other] { $agent } wants to delete { $count } items from the trash for good. Allow?
    }
mcp-confirm-delete-tasks = { $count ->
        [one] { $agent } wants to move one task to the trash. Allow?
       *[other] { $agent } wants to move { $count } tasks, with their subtasks, to the trash. Allow?
    }
mcp-confirm-declined = The person didn't allow { $tool }.
mcp-confirm-declined-hint = Don't ask again for the same change unless the person tells you to.
mcp-confirm-stale = The answer to the question about { $tool } doesn't belong to this call, or came too late.
mcp-confirm-stale-hint = Call the tool again without a request state to ask again.

## Resources

mcp-no-such-resource = There is no resource "{ $uri }".
mcp-no-such-resource-hint = Use glyphkeel://tasks/KEY, glyphkeel://projects/KEY or glyphkeel://queries?query=QUERY.
mcp-too-many-subscriptions = { $max ->
        [one] This session watches one resource, the most allowed.
       *[other] This session watches { $max } resources, the most allowed.
    }

## The HTTP endpoint

mcp-http-no-token = The HTTP endpoint needs a token.
mcp-http-port-taken = Port { $port } is in use by another program.
mcp-http-failed = The HTTP endpoint couldn't start on port { $port }: { $error }
mcp-http-listening = Serving MCP at { $url }
mcp-http-token-created = Created a token for the HTTP endpoint and saved it in { $file }.
mcp-http-token-where = Clients send the token from { $file } (agent_http.token) as "Authorization: Bearer <token>". Press Ctrl+C to stop.
mcp-http-no-settings-file = No personal settings file: HOME is not set.
