"""Frozen Issue #442 HTTP + file oracles for a disposable app copy.

Run against an already started generated app using --base-url, --workspace and
--adapter. The workspace must contain .issue442-oracle-scratch and must not be
a historical session. Cases restore all managed JSON bytes and modes, including
related files. An exception or unavailable server is a failure, never a pass.
"""

import argparse
import concurrent.futures
import contextlib
import json
import shutil
import stat
import urllib.error
import urllib.request
from pathlib import Path

CASES = Path(__file__).resolve().parents[1] / "tests/corpus/apps/nextjs-domain-oracles/cases.json"


class Oracle:
    def __init__(self, base_url, workspace, adapter, arm_race=None):
        self.url = base_url.rstrip("/")
        self.root = Path(workspace).resolve()
        if not (self.root / ".issue442-oracle-scratch").is_file():
            raise ValueError("requires a marked disposable oracle workspace")
        if "sessions" in self.root.parts or "workspace/management/runs" in str(self.root):
            raise ValueError("historical evidence is read-only")
        self.a = adapter
        self.cases = json.loads(CASES.read_text())
        self.arm_race = arm_race
        self.names = [adapter["parent"], adapter["child"]]
        if any(name not in {"staff", "shifts", "products", "orders", "departments", "expenses"} for name in self.names):
            raise ValueError("unsupported file adapter")
        self.paths = [self.root / "data" / (name + ".json") for name in self.names]
        if (self.root / "data").is_symlink() or any(p.is_symlink() for p in self.paths):
            raise ValueError("oracle data paths must not be symlinks")
        self.results = []

    def request(self, method, name, body=None):
        path = name if name.startswith("/") else "/api/" + name
        request = urllib.request.Request(
            self.url + path,
            data=None if body is None else json.dumps(body).encode(),
            method=method,
            headers={"Content-Type": "application/json", "Cache-Control": "no-cache"},
        )
        try:
            response = urllib.request.urlopen(request, timeout=10)
        except urllib.error.HTTPError as error:
            response = error
        with response:
            raw = response.read()
            try:
                value = json.loads(raw)
            except (json.JSONDecodeError, UnicodeDecodeError):
                value = {"unparsed": raw.decode(errors="replace")[:200]}
            return response.status, value

    def unwrap(self, name, value, collection=False):
        if name not in self.a["wrapped_collections"]:
            return value
        key = name if collection or name == "staff" else "shift"
        return value[key]

    def create(self, name, body):
        status, value = self.request("POST", name, body)
        assert status == 201, ("setup create", name, status, value)
        item = self.unwrap(name, value)
        assert item.get("id"), ("setup ID missing", name, value)
        return item

    def snapshot(self):
        result = {}
        for path in self.paths:
            if not path.exists():
                result[path.name] = None
            elif path.is_dir():
                result[path.name] = ("dir", stat.S_IMODE(path.stat().st_mode), {
                    str(p.relative_to(path)): p.read_bytes() for p in path.rglob("*") if p.is_file()
                })
            else:
                result[path.name] = ("file", stat.S_IMODE(path.stat().st_mode), path.read_bytes())
        return result

    def restore(self, saved):
        for path in self.paths:
            if path.is_dir():
                shutil.rmtree(path)
            elif path.exists():
                path.chmod(0o600)
                path.unlink()
            entry = saved[path.name]
            if entry is None:
                continue
            kind, mode, content = entry
            path.parent.mkdir(parents=True, exist_ok=True)
            if kind == "dir":
                path.mkdir()
                for name, data in content.items():
                    target = path / name
                    target.parent.mkdir(parents=True, exist_ok=True)
                    target.write_bytes(data)
            else:
                path.write_bytes(content)
            path.chmod(mode)

    @contextlib.contextmanager
    def isolated(self):
        before = self.snapshot()
        try:
            yield
        finally:
            self.restore(before)
            assert self.snapshot() == before, "oracle restoration failed"

    def record(self, name, callback):
        with self.isolated():
            try:
                detail = callback()
                self.results.append({"case": name, "passed": True, "detail": detail})
            except (AssertionError, OSError, ValueError, KeyError, TypeError) as error:
                self.results.append({"case": name, "passed": False, "detail": str(error)})

    def disk(self, name):
        return json.loads((self.root / "data" / (name + ".json")).read_text())

    def consistent(self, name):
        status, value = self.request("GET", name)
        assert status == 200, (name, status, value)
        rows = self.unwrap(name, value, collection=True)
        assert sorted(rows, key=lambda x: x["id"]) == sorted(self.disk(name), key=lambda x: x["id"])
        return rows

    def child_body(self, parent_id):
        domain = self.a["domain"]
        if domain == "inventory":
            return {"customerName": "oracle-client", "items": [{"productId": parent_id, "quantity": 1}]}
        if domain == "expense":
            return {"applicant": "oracle-applicant", "departmentId": parent_id,
                    "date": self.cases["expense"]["months"][0], "category": "交通費", "amount": 100,
                    "purpose": "oracle-persisted-record"}
        case = self.cases["shift"]
        return {"staffId": parent_id, "date": case["date"], "startTime": case["start"],
                "endTime": case["end"], "breakMinutes": 0}

    def rejection(self, method, name, body):
        before = self.snapshot()
        status, value = self.request(method, name, body)
        assert 400 <= status < 500, ("must reject", status, value)
        assert self.snapshot() == before, "rejected operation changed stored bytes/modes"

    def protection(self, name, fault):
        path = self.root / "data" / (name + ".json")
        if fault == "corrupt":
            path.write_bytes(b'{"broken":')
        elif fault == "directory":
            path.unlink()
            path.mkdir()
            (path / "sentinel").write_bytes(b"preserve directory contents")
        else:
            path.chmod(0o444)
        before = self.snapshot()
        statuses = []
        if fault != "readonly":
            statuses.append(self.request("GET", name)[0])
        body = dict(self.a["parent_body"]) if name == self.a["parent"] else self.child_body(self.parent["id"])
        if name == "shifts":
            body["date"] = "2026-09-15"
        if name == "products":
            body["sku"] = "ORACLE-FAULT"
        statuses.append(self.request("POST", name, body)[0])
        unchanged = self.snapshot() == before
        assert all(500 <= status < 600 for status in statuses) and unchanged, {
            "statuses": statuses, "all_related_bytes_modes_preserved": unchanged}
        return {"statuses": statuses, "all_related_bytes_modes_preserved": unchanged}

    def empty(self):
        for path in self.paths:
            path.write_text("[]")
        before = self.snapshot()
        for name in self.names:
            assert self.consistent(name) == [], "valid empty data was re-seeded"
        assert self.snapshot() == before

    def concurrent(self):
        name = self.a["parent"]
        before = self.consistent(name)
        bodies = [{**self.a["parent_body"], "name": "oracle-concurrent-" + str(i)} for i in range(2)]
        if name == "products":
            for i, body in enumerate(bodies):
                body["sku"] = "ORACLE-RACE-" + str(i)
        if self.arm_race:
            self.arm_race()
        with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
            replies = list(pool.map(lambda body: self.request("POST", name, body), bodies))
        assert [status for status, _ in replies] == [201, 201], replies
        ids = [self.unwrap(name, value)["id"] for _, value in replies]
        after = self.consistent(name)
        expected = {row["id"] for row in before} | set(ids)
        assert len(set(ids)) == 2 and {row["id"] for row in after} == expected, {
            "statuses": [201, 201], "expected_count": len(expected), "saved_count": len(after)}
        return {"statuses": [201, 201], "saved_count": len(after)}

    def inventory(self, quantities, reject):
        child = self.create("orders", {"customerName": "oracle-inventory",
            "items": [{"productId": self.parent["id"], "quantity": q} for q in quantities]})
        route = "orders?id=" + child["id"]
        if reject:
            self.rejection("PUT", route, {"status": "confirmed"})
        else:
            status, body = self.request("PUT", route, {"status": "confirmed"})
            assert status == 200 and body["status"] == "confirmed", (status, body)
            assert next(row for row in self.consistent("products") if row["id"] == self.parent["id"])["stock"] == 0
        assert all(row["stock"] >= 0 for row in self.disk("products"))

    def shift(self, change, reject):
        body = {**self.child_body(self.parent["id"]), **change}
        if reject:
            self.rejection("POST", "shifts", body)
        else:
            item = self.create("shifts", body)
            assert item in self.consistent("shifts")

    def parent_delete(self):
        before = self.snapshot()
        route = "staff/" + self.parent["id"] if self.a["delete_style"] == "path" else "staff"
        status, value = self.request("DELETE", route, None if self.a["delete_style"] == "path" else {"id": self.parent["id"]})
        if 400 <= status < 500:
            assert self.snapshot() == before
        else:
            assert 200 <= status < 300, (status, value)
            parents = {row["id"] for row in self.consistent("staff")}
            assert all(row["staffId"] in parents for row in self.consistent("shifts")), "orphan shifts after parent deletion"

    def budget(self, monthly):
        case = self.cases["expense"]
        for amount in case["exact_parts"]:
            item = self.create("expenses", {**self.child_body(self.parent["id"]), "amount": amount})
            status, value = self.request("PUT", "expenses", {"id": item["id"], "action": "approve"})
            assert status == 200 and value["status"] == "承認", (status, value)
        item = self.create("expenses", {**self.child_body(self.parent["id"]),
            "date": case["months"][1 if monthly else 0], "amount": case["budget"] if monthly else case["one_over"]})
        body = {"id": item["id"], "action": "approve"}
        if monthly:
            status, value = self.request("PUT", "expenses", body)
            assert status == 200 and value["status"] == "承認", (status, value)
        else:
            self.rejection("PUT", "expenses", body)
        self.consistent("expenses")

    def run(self):
        with self.isolated():
            self.parent = self.create(self.a["parent"], self.a["parent_body"])
            child = self.create(self.a["child"], self.child_body(self.parent["id"]))
            assert self.parent in self.consistent(self.a["parent"])
            assert child in self.consistent(self.a["child"])
            for name in self.names:
                for fault in ["corrupt", "directory", "readonly"]:
                    self.record(fault + "-" + name, lambda name=name, fault=fault: self.protection(name, fault))
            self.record("empty-no-seed", self.empty)
            self.record("concurrent-create", self.concurrent)
            if self.a["domain"] == "inventory":
                for name, quantities in self.cases["inventory"]["quantities"].items():
                    self.record(name, lambda quantities=quantities: self.inventory(quantities, sum(quantities) > self.cases["inventory"]["stock"]))
            elif self.a["domain"] == "shift":
                case = self.cases["shift"]
                self.record("parent-delete", self.parent_delete)
                self.record("break-equal", lambda: self.shift({"date": "2026-09-15", "breakMinutes": case["break_equal"]}, True))
                self.record("shift-misaligned", lambda: self.shift({"startTime": case["misaligned"]}, True))
                self.record("shift-aligned", lambda: self.shift({"date": "2026-09-15", "startTime": case["aligned"]}, False))
                self.record("shift-overlap", lambda: self.shift({"startTime": case["overlap_start"], "endTime": case["next_end"]}, True))
                self.record("shift-adjacent", lambda: self.shift({"startTime": case["adjacent_start"], "endTime": case["next_end"]}, False))
            else:
                for name, amount in self.cases["expense"]["invalid_amounts"].items():
                    self.record(name, lambda amount=amount: self.rejection("POST", "expenses", {**self.child_body(self.parent["id"]), "amount": amount}))
                self.record("budget-exact-and-over", lambda: self.budget(False))
                self.record("budget-month", lambda: self.budget(True))
        return self.results


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base-url", required=True)
    parser.add_argument("--workspace", required=True, type=Path)
    parser.add_argument("--adapter", required=True, type=Path)
    args = parser.parse_args()
    results = Oracle(args.base_url, args.workspace, json.loads(args.adapter.read_text())).run()
    print(json.dumps(results, ensure_ascii=False, indent=2))
    return 0 if all(result["passed"] for result in results) else 1


if __name__ == "__main__":
    raise SystemExit(main())
