#!/usr/bin/env python3
"""Build the Pi-hole Grafana v2 dashboard (klausk.xyz) with v6 exporter metrics."""

from __future__ import annotations

import copy
import json
import sys
from pathlib import Path
from typing import Any

DATASOURCE = {"name": "R5_IOpiMz"}
HOSTNAME_RENAME = {
    "kind": "Transformation",
    "group": "renameByRegex",
    "spec": {
        "options": {
            "regex": "([^.]+)\\.klausk\\.xyz",
            "renamePattern": "$1",
        }
    },
}
VIZ_VERSION = "13.0.1+security-01"
SERVER = '$server'
RASPBERRY_PI_LOGO = (
    "https://cdn.jsdelivr.net/gh/walkxcode/dashboard-icons/png/raspberry-pi.png"
)
OVERVIEW_ROW_TITLE = (
    f'<img src="{RASPBERRY_PI_LOGO}" width="20" height="20" '
    'style="vertical-align:middle;margin-right:6px;"/> Pi-hole Overview'
)


def prom_query(expr: str, **kwargs: Any) -> dict[str, Any]:
    spec: dict[str, Any] = {"expr": expr}
    spec.update(kwargs)
    return {
        "kind": "DataQuery",
        "group": "prometheus",
        "version": "v0",
        "datasource": DATASOURCE,
        "spec": spec,
    }


def panel_query(expr: str, ref_id: str = "A", **kwargs: Any) -> dict[str, Any]:
    return {
        "kind": "PanelQuery",
        "spec": {
            "query": prom_query(expr, **kwargs),
            "refId": ref_id,
            "hidden": False,
        },
    }


def timeseries_defaults(stacking: str = "none") -> dict[str, Any]:
    return {
        "defaults": {
            "unit": "short",
            "thresholds": {
                "mode": "absolute",
                "steps": [
                    {"value": 0, "color": "green"},
                    {"value": 80, "color": "red"},
                ],
            },
            "color": {"mode": "palette-classic"},
            "custom": {
                "axisBorderShow": False,
                "axisCenteredZero": False,
                "axisColorMode": "text",
                "axisLabel": "",
                "axisPlacement": "auto",
                "barAlignment": 0,
                "barWidthFactor": 0.6,
                "drawStyle": "line",
                "fillOpacity": 10,
                "gradientMode": "none",
                "hideFrom": {"legend": False, "tooltip": False, "viz": False},
                "insertNulls": False,
                "lineInterpolation": "linear",
                "lineWidth": 1,
                "pointSize": 5,
                "scaleDistribution": {"type": "linear"},
                "showPoints": "never",
                "showValues": False,
                "spanNulls": False,
                "stacking": {"group": "A", "mode": stacking},
                "thresholdsStyle": {"mode": "off"},
            },
        },
        "overrides": [],
    }


def timeseries_panel(
    panel_id: int,
    title: str,
    queries: list[dict[str, Any]],
    *,
    unit: str = "short",
    stacking: str = "none",
    legend: bool = True,
    transforms: list[dict[str, Any]] | None = None,
) -> dict[str, Any]:
    t = [copy.deepcopy(HOSTNAME_RENAME)]
    if transforms:
        t.extend(transforms)
    return {
        "kind": "Panel",
        "spec": {
            "id": panel_id,
            "title": title,
            "description": "",
            "links": [],
            "data": {
                "kind": "QueryGroup",
                "spec": {
                    "queries": queries,
                    "transformations": t,
                    "queryOptions": {},
                },
            },
            "vizConfig": {
                "kind": "VizConfig",
                "group": "timeseries",
                "version": VIZ_VERSION,
                "spec": {
                    "options": {
                        "annotations": {"clustering": -1, "multiLane": False},
                        "legend": {
                            "calcs": [],
                            "displayMode": "list",
                            "placement": "bottom",
                            "showLegend": legend,
                        },
                        "tooltip": {
                            "hideZeros": False,
                            "mode": "multi",
                            "sort": "none",
                        },
                    },
                    "fieldConfig": timeseries_defaults(stacking),
                },
            },
            "transparent": True,
        },
    }


def stat_panel(
    panel_id: int,
    title: str,
    expr: str,
    *,
    unit: str = "short",
    color_mode: str = "none",
    mappings: list[dict[str, Any]] | None = None,
    thresholds: list[dict[str, Any]] | None = None,
) -> dict[str, Any]:
    field_defaults: dict[str, Any] = {
        "unit": unit,
        "thresholds": {
            "mode": "absolute",
            "steps": thresholds or [{"value": 0, "color": "blue"}],
        },
        "color": {"mode": "thresholds"},
    }
    if mappings:
        field_defaults["mappings"] = mappings
    return {
        "kind": "Panel",
        "spec": {
            "id": panel_id,
            "title": title,
            "description": "",
            "links": [],
            "data": {
                "kind": "QueryGroup",
                "spec": {
                    "queries": [
                        panel_query(expr, instant=True, legendFormat=title),
                    ],
                    "transformations": [copy.deepcopy(HOSTNAME_RENAME)],
                    "queryOptions": {},
                },
            },
            "vizConfig": {
                "kind": "VizConfig",
                "group": "stat",
                "version": VIZ_VERSION,
                "spec": {
                    "options": {
                        "colorMode": color_mode,
                        "graphMode": "none",
                        "justifyMode": "center" if color_mode == "none" else "auto",
                        "orientation": "auto",
                        "percentChangeColorMode": "standard",
                        "reduceOptions": {
                            "calcs": ["lastNotNull"],
                            "fields": "",
                            "values": False,
                        },
                        "showPercentChange": False,
                        "textMode": "auto",
                        "wideLayout": True,
                    },
                    "fieldConfig": {
                        "defaults": field_defaults,
                        "overrides": [],
                    },
                },
            },
            "transparent": True,
        },
    }


def pie_panel(panel_id: int, title: str, expr: str, *, legend_format: str) -> dict[str, Any]:
    return {
        "kind": "Panel",
        "spec": {
            "id": panel_id,
            "title": title,
            "description": "",
            "links": [],
            "data": {
                "kind": "QueryGroup",
                "spec": {
                    "queries": [
                        panel_query(
                            expr,
                            editorMode="code",
                            instant=True,
                            legendFormat=legend_format,
                        )
                    ],
                    "transformations": [copy.deepcopy(HOSTNAME_RENAME)],
                    "queryOptions": {"maxDataPoints": 20},
                },
            },
            "vizConfig": {
                "kind": "VizConfig",
                "group": "piechart",
                "version": VIZ_VERSION,
                "spec": {
                    "options": {
                        "displayLabels": ["percent"],
                        "legend": {
                            "displayMode": "table",
                            "placement": "right",
                            "showLegend": True,
                            "values": ["value"],
                        },
                        "pieType": "donut",
                        "reduceOptions": {
                            "calcs": ["lastNotNull"],
                            "fields": "",
                            "values": False,
                        },
                        "sort": "desc",
                        "tooltip": {
                            "hideZeros": False,
                            "mode": "single",
                            "sort": "none",
                        },
                    },
                    "fieldConfig": {
                        "defaults": {
                            "unit": "none",
                            "decimals": 0,
                            "color": {"mode": "palette-classic"},
                            "custom": {
                                "hideFrom": {
                                    "legend": False,
                                    "tooltip": False,
                                    "viz": False,
                                }
                            },
                        },
                        "overrides": [],
                    },
                },
            },
            "transparent": True,
        },
    }


def table_panel(
    panel_id: int,
    title: str,
    expr: str,
    *,
    organize: dict[str, Any],
    sort_field: str,
    threshold_color: str = "blue",
) -> dict[str, Any]:
    return {
        "kind": "Panel",
        "spec": {
            "id": panel_id,
            "title": title,
            "description": "",
            "links": [],
            "data": {
                "kind": "QueryGroup",
                "spec": {
                    "queries": [
                        panel_query(
                            expr,
                            editorMode="code",
                            format="table",
                            instant=True,
                        )
                    ],
                    "transformations": [
                        copy.deepcopy(HOSTNAME_RENAME),
                        {
                            "kind": "Transformation",
                            "group": "organize",
                            "spec": {"options": organize},
                        },
                    ],
                    "queryOptions": {},
                },
            },
            "vizConfig": {
                "kind": "VizConfig",
                "group": "table",
                "version": VIZ_VERSION,
                "spec": {
                    "options": {
                        "cellHeight": "sm",
                        "showHeader": True,
                        "sortBy": [{"desc": True, "displayName": sort_field}],
                    },
                    "fieldConfig": {
                        "defaults": {
                            "unit": "short",
                            "thresholds": {
                                "mode": "absolute",
                                "steps": [{"value": 0, "color": threshold_color}],
                            },
                            "color": {"mode": "thresholds"},
                            "custom": {
                                "align": "auto",
                                "cellOptions": {"type": "auto"},
                                "footer": {"reducers": []},
                                "inspect": False,
                            },
                        },
                        "overrides": [],
                    },
                },
            },
            "transparent": True,
        },
    }


def gauge_panel(panel_id: int, title: str, expr: str) -> dict[str, Any]:
    return {
        "kind": "Panel",
        "spec": {
            "id": panel_id,
            "title": title,
            "description": "",
            "links": [],
            "data": {
                "kind": "QueryGroup",
                "spec": {
                    "queries": [
                        panel_query(
                            expr,
                            editorMode="code",
                            instant=False,
                            legendFormat="% blocked",
                            range=True,
                        )
                    ],
                    "transformations": [copy.deepcopy(HOSTNAME_RENAME)],
                    "queryOptions": {},
                },
            },
            "vizConfig": {
                "kind": "VizConfig",
                "group": "gauge",
                "version": VIZ_VERSION,
                "spec": {
                    "options": {
                        "reduceOptions": {
                            "calcs": ["lastNotNull"],
                            "fields": "",
                            "values": False,
                        },
                        "shape": "gauge",
                        "showThresholdLabels": True,
                        "showThresholdMarkers": True,
                    },
                    "fieldConfig": {
                        "defaults": {
                            "unit": "percentunit",
                            "decimals": 1,
                            "min": 0,
                            "max": 1,
                            "thresholds": {
                                "mode": "percentage",
                                "steps": [
                                    {"value": 0, "color": "dark-red"},
                                    {"value": 30, "color": "yellow"},
                                    {"value": 50, "color": "semi-dark-green"},
                                ],
                            },
                            "color": {"mode": "thresholds"},
                        },
                        "overrides": [],
                    },
                },
            },
            "transparent": True,
        },
    }


def grid_item(name: str, x: int, y: int, width: int, height: int) -> dict[str, Any]:
    return {
        "kind": "GridLayoutItem",
        "spec": {
            "x": x,
            "y": y,
            "width": width,
            "height": height,
            "element": {"kind": "ElementReference", "name": name},
        },
    }


def row(title: str, items: list[dict[str, Any]]) -> dict[str, Any]:
    return {
        "kind": "RowsLayoutRow",
        "spec": {
            "title": title,
            "collapse": False,
            "layout": {"kind": "GridLayout", "spec": {"items": items}},
        },
    }


def build_elements() -> dict[str, Any]:
    h = f'hostname=~"{SERVER}"'
    return {
        "panel-1": stat_panel(
            1,
            "Status",
            f"pihole_status{{{h}}}",
            color_mode="background",
            mappings=[
                {
                    "type": "value",
                    "options": {
                        "0": {"text": "Disabled", "color": "red"},
                        "1": {"text": "Enabled", "color": "green"},
                    },
                }
            ],
            thresholds=[
                {"value": 0, "color": "red"},
                {"value": 1, "color": "green"},
            ],
        ),
        "panel-2": stat_panel(
            2,
            "DNS Queries Today",
            f"sum(pihole_dns_queries_today{{{h}}})",
        ),
        "panel-3": stat_panel(
            3,
            "Ads Blocked Today",
            f"sum(pihole_ads_blocked_today{{{h}}})",
            thresholds=[{"value": 0, "color": "green"}],
        ),
        "panel-4": gauge_panel(
            4,
            "% Ads Blocked (across servers)",
            f"sum(pihole_ads_blocked_today{{{h}}}) / sum(pihole_dns_queries_today{{{h}}})",
        ),
        "panel-5": pie_panel(
            5,
            "Top Queries by Domain",
            f'topk(10, sum by (domain) (pihole_top_queries{{{h}}}))',
            legend_format="{{domain}}",
        ),
        "panel-6": pie_panel(
            6,
            "Top Ads by Domain",
            f'topk(10, sum by (domain) (pihole_top_ads{{{h}}}))',
            legend_format="{{domain}}",
        ),
        "panel-7": stat_panel(
            7,
            "Domains Being Blocked",
            f"sum(pihole_domains_being_blocked{{{h}}})",
        ),
        "panel-8": stat_panel(
            8,
            "Unique Domains Seen",
            f"max(pihole_unique_domains{{{h}}})",
        ),
        "panel-9": stat_panel(
            9,
            "Unique Clients (24h)",
            f"max(pihole_unique_clients{{{h}}})",
        ),
        "panel-10": timeseries_panel(
            10,
            "DNS Queries (all types)",
            [
                panel_query(
                    f"pihole_dns_queries_all_types{{{h}}}",
                    legendFormat="{{hostname}}",
                )
            ],
            legend=False,
        ),
        "panel-11": timeseries_panel(
            11,
            "Queries Cached / Forwarded",
            [
                panel_query(
                    f"pihole_queries_cached{{{h}}}",
                    legendFormat="Cached — {{hostname}}",
                ),
                panel_query(
                    f"pihole_queries_forwarded{{{h}}}",
                    ref_id="B",
                    legendFormat="Forwarded — {{hostname}}",
                ),
            ],
            stacking="normal",
        ),
        "panel-12": timeseries_panel(
            12,
            "Ads Blocked Today",
            [
                panel_query(
                    f"pihole_ads_blocked_today{{{h}}}",
                    legendFormat="{{hostname}}",
                )
            ],
            legend=False,
        ),
        "panel-13": timeseries_panel(
            13,
            "% Ads Blocked Over Time",
            [
                panel_query(
                    f"pihole_ads_percentage_today{{{h}}}",
                    legendFormat="{{hostname}}",
                )
            ],
            unit="percent",
        ),
        "panel-14": timeseries_panel(
            14,
            "Forward Destinations",
            [
                panel_query(
                    f"pihole_forward_destinations{{{h}}}",
                    legendFormat="{{destination_name}} — {{hostname}}",
                )
            ],
        ),
        "panel-15": timeseries_panel(
            15,
            "Unique Clients",
            [
                panel_query(
                    f"pihole_unique_clients{{{h}}}",
                    legendFormat="{{hostname}}",
                )
            ],
        ),
        "panel-20": timeseries_panel(
            20,
            "DNS Query Types",
            [
                panel_query(
                    f'sum by(type) (pihole_querytypes{{{h}, type!~"ANY|DNSKEY|DS|MX|NAPTR|RRSIG"}})',
                    editorMode="code",
                    legendFormat="{{type}}",
                    range=True,
                )
            ],
        ),
        "panel-21": {
            "kind": "Panel",
            "spec": {
                "id": 21,
                "title": "Reply Types",
                "description": "",
                "links": [],
                "data": {
                    "kind": "QueryGroup",
                    "spec": {
                        "queries": [
                            panel_query(
                                f'sum by (type) (pihole_reply{{{h}}} > 0)',
                                instant=True,
                                legendFormat="{{type}}",
                            )
                        ],
                        "transformations": [copy.deepcopy(HOSTNAME_RENAME)],
                        "queryOptions": {"maxDataPoints": 20},
                    },
                },
                "vizConfig": {
                    "kind": "VizConfig",
                    "group": "piechart",
                    "version": VIZ_VERSION,
                    "spec": {
                        "options": {
                            "displayLabels": ["name", "percent"],
                            "legend": {
                                "displayMode": "table",
                                "placement": "right",
                                "showLegend": True,
                                "values": ["percent"],
                            },
                            "pieType": "donut",
                            "reduceOptions": {
                                "calcs": ["lastNotNull"],
                                "fields": "",
                                "values": False,
                            },
                            "sort": "desc",
                            "tooltip": {
                                "hideZeros": False,
                                "mode": "single",
                                "sort": "none",
                            },
                        },
                        "fieldConfig": {
                            "defaults": {
                                "unit": "short",
                                "decimals": 2,
                                "color": {"mode": "palette-classic"},
                                "custom": {
                                    "hideFrom": {
                                        "legend": False,
                                        "tooltip": False,
                                        "viz": False,
                                    }
                                },
                            },
                            "overrides": [],
                        },
                    },
                },
                "transparent": True,
            },
        },
        "panel-22": timeseries_panel(
            22,
            "Upstream Response Time",
            [
                panel_query(
                    f'pihole_forward_destinations_responsetime{{{h}, destination_name!~"blocklist|cache"}}',
                    legendFormat="{{destination_name}} — {{hostname}}",
                )
            ],
            unit="s",
        ),
        "panel-30": table_panel(
            30,
            "Top Blocked Domains",
            f'topk(15, sum by (domain) (pihole_top_ads{{{h}}}))',
            organize={
                "excludeByName": {
                    "Time": True,
                    "__name__": True,
                    "instance": True,
                    "job": True,
                },
                "renameByName": {
                    "Value": "Hits",
                    "domain": "Domain",
                    "hostname": "Hostname",
                },
            },
            sort_field="Hits",
            threshold_color="red",
        ),
        "panel-31": table_panel(
            31,
            "Top Queried Domains",
            f'topk(15, sum by (domain) (pihole_top_queries{{{h}}}))',
            organize={
                "excludeByName": {
                    "Time": True,
                    "__name__": True,
                    "instance": True,
                    "job": True,
                },
                "renameByName": {
                    "Value": "Queries",
                    "domain": "Domain",
                    "hostname": "Hostname",
                },
            },
            sort_field="Queries",
        ),
        "panel-32": {
            "kind": "Panel",
            "spec": {
                "id": 32,
                "title": "Top Query Sources",
                "description": "",
                "links": [],
                "data": {
                    "kind": "QueryGroup",
                    "spec": {
                        "queries": [
                            panel_query(
                                f'topk(15, sum by (source_name, source) (pihole_top_sources{{{h}}}))',
                                editorMode="code",
                                format="table",
                                instant=True,
                                range=False,
                            )
                        ],
                        "transformations": [
                            {
                                "kind": "Transformation",
                                "group": "filterFieldsByName",
                                "spec": {
                                    "options": {
                                        "include": {
                                            "names": [
                                                "source_name",
                                                "Value",
                                                "source",
                                            ]
                                        }
                                    }
                                },
                            }
                        ],
                        "queryOptions": {},
                    },
                },
                "vizConfig": {
                    "kind": "VizConfig",
                    "group": "table",
                    "version": VIZ_VERSION,
                    "spec": {
                        "options": {
                            "cellHeight": "sm",
                            "showHeader": True,
                            "sortBy": [{"desc": True, "displayName": "Value"}],
                        },
                        "fieldConfig": {
                            "defaults": {
                                "unit": "none",
                                "thresholds": {
                                    "mode": "absolute",
                                    "steps": [{"value": 0, "color": "purple"}],
                                },
                                "color": {"mode": "thresholds"},
                                "custom": {
                                    "align": "auto",
                                    "cellOptions": {"type": "auto"},
                                    "filterable": False,
                                    "footer": {"reducers": []},
                                    "inspect": False,
                                },
                            },
                            "overrides": [],
                        },
                    },
                },
                "transparent": True,
            },
        },
        # v6 exporter metrics
        "panel-40": stat_panel(
            40,
            "Exporter Scrape OK",
            f"min(pihole_scrape_success{{{h}}})",
            mappings=[
                {
                    "type": "value",
                    "options": {
                        "0": {"text": "Failed", "color": "red"},
                        "1": {"text": "OK", "color": "green"},
                    },
                }
            ],
            thresholds=[
                {"value": 0, "color": "red"},
                {"value": 1, "color": "green"},
            ],
        ),
        "panel-41": timeseries_panel(
            41,
            "Scrape Duration",
            [
                panel_query(
                    f"pihole_scrape_duration_seconds{{{h}}}",
                    legendFormat="{{hostname}}",
                )
            ],
            unit="s",
        ),
        "panel-42": timeseries_panel(
            42,
            "Query Processing Status",
            [
                panel_query(
                    f"sum by (status) (pihole_query_status{{{h}}})",
                    legendFormat="{{status}}",
                )
            ],
            stacking="normal",
        ),
        "panel-43": timeseries_panel(
            43,
            "24h Query History (10 min slots)",
            [
                panel_query(
                    f'pihole_history{{{h}, field="total"}}',
                    legendFormat="total — {{hostname}}",
                ),
                panel_query(
                    f'pihole_history{{{h}, field="blocked"}}',
                    ref_id="B",
                    legendFormat="blocked — {{hostname}}",
                ),
                panel_query(
                    f'pihole_history{{{h}, field="cached"}}',
                    ref_id="C",
                    legendFormat="cached — {{hostname}}",
                ),
            ],
        ),
        "panel-44": stat_panel(
            44,
            "Gravity Age",
            f"max(pihole_gravity_age_seconds{{{h}}})",
            unit="s",
        ),
        "panel-45": timeseries_panel(
            45,
            "Request Rate",
            [
                panel_query(
                    f"pihole_request_rate{{{h}}}",
                    legendFormat="{{hostname}}",
                )
            ],
            unit="reqps",
        ),
        "panel-46": timeseries_panel(
            46,
            "System RAM Used %",
            [
                panel_query(
                    f"pihole_system_ram_used_percent{{{h}}}",
                    legendFormat="{{hostname}}",
                )
            ],
            unit="percent",
        ),
        "panel-47": timeseries_panel(
            47,
            "System CPU %",
            [
                panel_query(
                    f"pihole_system_cpu_percent{{{h}}}",
                    legendFormat="{{hostname}}",
                )
            ],
            unit="percent",
        ),
        "panel-48": timeseries_panel(
            48,
            "FTL CPU / Memory %",
            [
                panel_query(
                    f"pihole_ftl_cpu_percent{{{h}}}",
                    legendFormat="CPU — {{hostname}}",
                ),
                panel_query(
                    f"pihole_ftl_mem_percent{{{h}}}",
                    ref_id="B",
                    legendFormat="Mem — {{hostname}}",
                ),
            ],
            unit="percent",
        ),
        "panel-49": timeseries_panel(
            49,
            "Load Average",
            [
                panel_query(
                    f"pihole_system_load{{{h}}}",
                    legendFormat="{{period}} — {{hostname}}",
                )
            ],
        ),
        "panel-50": timeseries_panel(
            50,
            "CPU Temperature",
            [
                panel_query(
                    f"pihole_cpu_temp{{{h}}}",
                    legendFormat="{{unit}} — {{hostname}}",
                )
            ],
            unit="celsius",
        ),
        "panel-51": table_panel(
            51,
            "Component Versions",
            f'pihole_version_info{{{h}}} == 1',
            organize={
                "excludeByName": {
                    "Time": True,
                    "__name__": True,
                    "Value": True,
                    "instance": True,
                    "job": True,
                },
                "renameByName": {
                    "branch": "Branch",
                    "component": "Component",
                    "hash": "Hash",
                    "hostname": "Hostname",
                    "version": "Version",
                },
            },
            sort_field="Component",
        ),
        "panel-52": stat_panel(
            52,
            "Query DB Size",
            f"max(pihole_database_size_bytes{{{h}}})",
            unit="bytes",
        ),
        "panel-53": timeseries_panel(
            53,
            "API Summary Latency",
            [
                panel_query(
                    f"pihole_api_summary_took_seconds{{{h}}}",
                    legendFormat="{{hostname}}",
                )
            ],
            unit="s",
        ),
        "panel-54": timeseries_panel(
            54,
            "Last 10 Minutes",
            [
                panel_query(
                    f"pihole_queries_last_10min{{{h}}}",
                    legendFormat="queries — {{hostname}}",
                ),
                panel_query(
                    f"pihole_ads_last_10min{{{h}}}",
                    ref_id="B",
                    legendFormat="blocked — {{hostname}}",
                ),
            ],
        ),
        "panel-55": stat_panel(
            55,
            "Clients Ever Seen",
            f"sum(pihole_clients_ever_seen{{{h}}})",
        ),
    }


def build_dashboard() -> dict[str, Any]:
    return {
        "apiVersion": "dashboard.grafana.app/v2",
        "kind": "Dashboard",
        "metadata": {
            "name": "pihole-klausk-v4",
            "namespace": "default",
            "uid": "ee4ded5c-bb2e-4693-90fe-f3dd2d48b6c4",
            "labels": {
                "grafana.app/deprecatedInternalID": "1170132250636288"
            },
        },
        "spec": {
            "annotations": [
                {
                    "kind": "AnnotationQuery",
                    "spec": {
                        "query": {
                            "kind": "DataQuery",
                            "group": "grafana",
                            "version": "v0",
                            "datasource": DATASOURCE,
                            "spec": {},
                        },
                        "enable": True,
                        "hide": True,
                        "iconColor": "rgba(0, 211, 255, 1)",
                        "name": "Annotations & Alerts",
                        "builtIn": True,
                    },
                }
            ],
            "cursorSync": "Crosshair",
            "description": "Pi-hole monitoring dashboard for klausk.xyz — includes v6 exporter metrics",
            "editable": True,
            "elements": build_elements(),
            "layout": {
                "kind": "RowsLayout",
                "spec": {
                    "rows": [
                        row(
                            OVERVIEW_ROW_TITLE,
                            [
                                grid_item("panel-1", 0, 0, 4, 4),
                                grid_item("panel-2", 4, 0, 4, 4),
                                grid_item("panel-3", 8, 0, 4, 4),
                                grid_item("panel-7", 12, 0, 4, 4),
                                grid_item("panel-8", 16, 0, 4, 4),
                                grid_item("panel-9", 20, 0, 4, 4),
                                grid_item("panel-4", 0, 4, 6, 9),
                                grid_item("panel-5", 6, 4, 9, 9),
                                grid_item("panel-6", 15, 4, 9, 9),
                            ],
                        ),
                        row(
                            "Exporter Health",
                            [
                                grid_item("panel-40", 0, 0, 4, 4),
                                grid_item("panel-41", 4, 0, 8, 4),
                                grid_item("panel-53", 12, 0, 6, 4),
                                grid_item("panel-55", 18, 0, 6, 4),
                            ],
                        ),
                        row(
                            "Query Traffic",
                            [
                                grid_item("panel-10", 0, 0, 8, 8),
                                grid_item("panel-11", 8, 0, 8, 8),
                                grid_item("panel-12", 16, 0, 8, 8),
                                grid_item("panel-13", 0, 8, 8, 8),
                                grid_item("panel-14", 8, 8, 8, 8),
                                grid_item("panel-15", 16, 8, 8, 8),
                            ],
                        ),
                        row(
                            "Query Processing (v6)",
                            [
                                grid_item("panel-42", 0, 0, 8, 8),
                                grid_item("panel-43", 8, 0, 16, 8),
                                grid_item("panel-45", 0, 8, 8, 8),
                                grid_item("panel-54", 8, 8, 8, 8),
                                grid_item("panel-44", 16, 8, 8, 8),
                            ],
                        ),
                        row(
                            "Query Types & Replies",
                            [
                                grid_item("panel-20", 0, 0, 10, 9),
                                grid_item("panel-21", 10, 0, 6, 9),
                                grid_item("panel-22", 16, 0, 8, 9),
                            ],
                        ),
                        row(
                            "System & FTL",
                            [
                                grid_item("panel-46", 0, 0, 8, 8),
                                grid_item("panel-47", 8, 0, 8, 8),
                                grid_item("panel-48", 16, 0, 8, 8),
                                grid_item("panel-49", 0, 8, 8, 8),
                                grid_item("panel-50", 8, 8, 8, 8),
                                grid_item("panel-52", 16, 8, 8, 8),
                            ],
                        ),
                        row(
                            "Top Lists",
                            [
                                grid_item("panel-30", 0, 0, 8, 12),
                                grid_item("panel-31", 8, 0, 8, 12),
                                grid_item("panel-32", 16, 0, 8, 12),
                            ],
                        ),
                        row(
                            "Versions",
                            [grid_item("panel-51", 0, 0, 24, 8)],
                        ),
                    ]
                },
            },
            "links": [],
            "liveNow": False,
            "preload": False,
            "tags": ["pihole", "dns", "networking"],
            "timeSettings": {
                "timezone": "browser",
                "from": "now-7d",
                "to": "now",
                "autoRefresh": "1m",
                "autoRefreshIntervals": [
                    "5s",
                    "10s",
                    "30s",
                    "1m",
                    "5m",
                    "15m",
                    "30m",
                    "1h",
                    "2h",
                    "1d",
                ],
                "hideTimepicker": False,
                "fiscalYearStartMonth": 0,
            },
            "title": "Pi-hole",
            "variables": [
                {
                    "kind": "QueryVariable",
                    "spec": {
                        "name": "server",
                        "current": {"text": "All", "value": ["$__all"]},
                        "label": "Server",
                        "hide": "dontHide",
                        "refresh": "onTimeRangeChanged",
                        "skipUrlSync": False,
                        "query": {
                            "kind": "DataQuery",
                            "group": "prometheus",
                            "version": "v0",
                            "datasource": DATASOURCE,
                            "spec": {
                                "qryType": 1,
                                "query": 'label_values(pihole_status{job="pihole"},hostname)',
                                "refId": "PrometheusVariableQueryEditor-VariableQuery",
                            },
                        },
                        "regex": "",
                        "regexApplyTo": "value",
                        "sort": "alphabeticalAsc",
                        "definition": 'label_values(pihole_status{job="pihole"},hostname)',
                        "options": [],
                        "multi": True,
                        "includeAll": True,
                        "allValue": ".*",
                        "allowCustomValue": True,
                    },
                },
                {
                    "kind": "AdhocVariable",
                    "group": "prometheus",
                    "datasource": DATASOURCE,
                    "spec": {
                        "name": "Filters",
                        "baseFilters": [],
                        "filters": [],
                        "defaultKeys": [],
                        "hide": "dontHide",
                        "skipUrlSync": False,
                        "allowCustomValue": True,
                        "enableGroupBy": False,
                    },
                },
            ],
        },
    }


def extend_dashboard(dashboard: dict[str, Any]) -> dict[str, Any]:
    """Merge v6 panels into an exported dashboard JSON."""
    out = copy.deepcopy(dashboard)
    out["metadata"]["generation"] = out["metadata"].get("generation", 0) + 1
    out["metadata"]["name"] = "pihole-klausk-v4"
    out["spec"]["description"] = (
        "Pi-hole monitoring dashboard for klausk.xyz — includes v6 exporter metrics"
    )
    built = build_elements()
    for key in (
        "panel-40",
        "panel-41",
        "panel-42",
        "panel-43",
        "panel-44",
        "panel-45",
        "panel-46",
        "panel-47",
        "panel-48",
        "panel-49",
        "panel-50",
        "panel-51",
        "panel-52",
        "panel-53",
        "panel-54",
        "panel-55",
    ):
        out["spec"]["elements"][key] = built[key]
    rows = out["spec"]["layout"]["spec"]["rows"]
    if not any(
        r.get("spec", {}).get("title") == "Exporter Health" for r in rows
    ):
        rows.insert(
            1,
            row(
                "Exporter Health",
                [
                    grid_item("panel-40", 0, 0, 4, 4),
                    grid_item("panel-41", 4, 0, 8, 4),
                    grid_item("panel-53", 12, 0, 6, 4),
                    grid_item("panel-55", 18, 0, 6, 4),
                ],
            ),
        )
    if not any(
        r.get("spec", {}).get("title") == "Query Processing (v6)" for r in rows
    ):
        idx = next(
            i
            for i, r in enumerate(rows)
            if r.get("spec", {}).get("title") == "Query Types & Replies"
        )
        rows.insert(
            idx,
            row(
                "Query Processing (v6)",
                [
                    grid_item("panel-42", 0, 0, 8, 8),
                    grid_item("panel-43", 8, 0, 16, 8),
                    grid_item("panel-45", 0, 8, 8, 8),
                    grid_item("panel-54", 8, 8, 8, 8),
                    grid_item("panel-44", 16, 8, 8, 8),
                ],
            ),
        )
    if not any(r.get("spec", {}).get("title") == "System & FTL" for r in rows):
        idx = next(
            i for i, r in enumerate(rows) if r.get("spec", {}).get("title") == "Top Lists"
        )
        rows.insert(
            idx,
            row(
                "System & FTL",
                [
                    grid_item("panel-46", 0, 0, 8, 8),
                    grid_item("panel-47", 8, 0, 8, 8),
                    grid_item("panel-48", 16, 0, 8, 8),
                    grid_item("panel-49", 0, 8, 8, 8),
                    grid_item("panel-50", 8, 8, 8, 8),
                    grid_item("panel-52", 16, 8, 8, 8),
                ],
            ),
        )
    if not any(r.get("spec", {}).get("title") == "Versions" for r in rows):
        rows.append(row("Versions", [grid_item("panel-51", 0, 0, 24, 8)]))
    for r in rows:
        title = r.get("spec", {}).get("title", "")
        if "Pi-hole Overview" in title and RASPBERRY_PI_LOGO not in title:
            r["spec"]["title"] = OVERVIEW_ROW_TITLE
    return out


def main() -> None:
    if len(sys.argv) == 1:
        dst = Path(__file__).with_name("dashboard.json")
        dashboard = build_dashboard()
    elif len(sys.argv) == 2:
        dst = Path(sys.argv[1])
        dashboard = build_dashboard()
    elif len(sys.argv) == 3:
        src, dst = Path(sys.argv[1]), Path(sys.argv[2])
        dashboard = extend_dashboard(json.loads(src.read_text()))
    else:
        print(
            f"usage: {sys.argv[0]} [output.json]\n"
            f"       {sys.argv[0]} <input.json> <output.json>\n"
            f"       {sys.argv[0]}   # writes dashboard.json next to this script",
            file=sys.stderr,
        )
        sys.exit(1)

    dst.write_text(json.dumps(dashboard, indent=2) + "\n")
    print(f"wrote {dst} ({len(dashboard['spec']['elements'])} panels)")


if __name__ == "__main__":
    main()
