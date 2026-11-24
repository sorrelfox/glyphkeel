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
