"""Port discovery from helm, kind-config, Tiltfile, bff-suite-config, openapi (RERP-style layout)."""

from brrtrouter_tooling.discovery.sources import (
    discover_bff_suite_config,
    discover_helm,
    discover_kind_host_ports,
    discover_openapi_bff_localhost,
    discover_openapi_suite_microservice_localhost,
    discover_tiltfile,
)
from brrtrouter_tooling.discovery.suites import (
    bff_service_to_suite,
    bff_suite_config_path,
    get_bff_service_name_from_config,
    iter_bffs,
    load_suite_services,
    openapi_bff_path,
    service_to_suite,
    suites_with_bff,
)

__all__ = [
    "bff_service_to_suite",
    "bff_suite_config_path",
    "discover_bff_suite_config",
    "discover_helm",
    "discover_kind_host_ports",
    "discover_openapi_bff_localhost",
    "discover_openapi_suite_microservice_localhost",
    "discover_tiltfile",
    "get_bff_service_name_from_config",
    "iter_bffs",
    "load_suite_services",
    "openapi_bff_path",
    "service_to_suite",
    "suites_with_bff",
]
