// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { getRouteApi, useNavigate } from "@tanstack/react-router";

import { DashboardsScreen } from "./DashboardsScreen";

const route = getRouteApi("/shell/dashboards");

/** The dashboards page, with the dashboard shown and the project it was opened from kept in the route. */
export function DashboardsRoute() {
  const { dashboard, project } = route.useSearch();
  const navigate = useNavigate();
  return (
    <DashboardsScreen
      chosen={dashboard}
      from={project}
      onChoose={(id) => {
        void navigate({ to: "/dashboards", search: { ...(id === undefined ? {} : { dashboard: id }), ...(project === undefined ? {} : { project }) } });
      }}
    />
  );
}
