// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

import { getRouteApi, useNavigate } from "@tanstack/react-router";

import { DashboardsScreen } from "./DashboardsScreen";

const route = getRouteApi("/shell/dashboards");

/** The dashboards page, with the dashboard shown kept in the route. */
export function DashboardsRoute() {
  const { dashboard } = route.useSearch();
  const navigate = useNavigate();
  return (
    <DashboardsScreen
      chosen={dashboard}
      onChoose={(id) => {
        void navigate({ to: "/dashboards", search: id === undefined ? {} : { dashboard: id } });
      }}
    />
  );
}
