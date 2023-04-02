#!/usr/bin/python3

import subprocess

def get_uri(uri):
    cmd = ["curl", "-ig", f"http://localhost:3000{uri}"]
    print("\n===", uri, "===")
    out = subprocess.check_output(cmd).decode()
    print(out)
    return out

res = get_uri("/redfish")
assert("200 OK" in res)
assert('{"v1":"/redfish/v1/"}' in res)

res = get_uri("/redfish/")
assert("200 OK" in res)
assert('{"v1":"/redfish/v1/"}' in res)

res = get_uri("/redfish/v1/")
assert("200 OK" in res)
assert('"@odata.id":"/redfish/v1"' in res)
assert('"@odata.type":"#ServiceRoot.v1_15_0.ServiceRoot"' in res)
assert('"Id":"RootService"' in res)
assert('"Name":"Root Service"' in res)

res = get_uri("/redfish/v1/NotFound")
assert("404 Not Found" in res)