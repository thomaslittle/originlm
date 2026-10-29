import copy
import importlib.util
import pathlib
import sys
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location(
    "publish_order", ROOT / "scripts" / "publish_order.py"
)
publish_order = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = publish_order
spec.loader.exec_module(publish_order)

EXPECTED = [
    "originlm-contracts",
    "originlm-platform",
    "originlm-core",
    "originlm-chatgpt-auth",
    "originlm-discovery",
    "originlm-runtime",
    "originlm-store",
    "originlm-service",
    "originlm-transports",
    "originlm-control",
    "originlm-adapters",
    "originlm-kit",
    "originlm",
]
EXCLUDED = {"originlm-cli", "originlm-desktop", "originlm-update"}


class PublishOrder(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.metadata = publish_order.load_metadata()

    def test_facade_closure_has_a_stable_dependency_first_order(self):
        order, packages = publish_order.validate(copy.deepcopy(self.metadata))
        self.assertEqual(order, EXPECTED)
        self.assertEqual(order[-1], "originlm")
        self.assertEqual(set(order), set(packages) - EXCLUDED)
        position = {name: index for index, name in enumerate(order)}
        for name in order:
            for dependency in packages[name].normal_dependencies:
                if dependency in package_names(self.metadata):
                    self.assertLess(
                        position[dependency],
                        position[name],
                        f"{dependency} must be published before {name}",
                    )
        # Determinism: repeated metadata reads produce the same order.
        again, _ = publish_order.validate(copy.deepcopy(self.metadata))
        self.assertEqual(order, again)

    def test_private_crates_keep_publish_false(self):
        packages = publish_order.workspace_packages(self.metadata)
        for name in EXCLUDED:
            self.assertIsNotNone(packages[name].publish, name)
            self.assertEqual(packages[name].publish, [], name)

    def test_cycle_fails_closed(self):
        packages = {
            "a": package("a", ("b",)),
            "b": package("b", ("a",)),
            "originlm": package("originlm", ("a",)),
        }
        with self.assertRaisesRegex(publish_order.PublishOrderError, "cycle"):
            publish_order.publishable_closure(packages, "originlm")

    def test_missing_version_or_path_fails_closed(self):
        for mutation in (drop_path, break_version):
            metadata = copy.deepcopy(self.metadata)
            dependency = next(
                item
                for item in next(
                    package
                    for package in metadata["packages"]
                    if package["name"] == "originlm-platform"
                )["dependencies"]
                if item["name"] == "originlm-contracts"
            )
            mutation(dependency)
            with self.subTest(mutation=mutation.__name__):
                with self.assertRaises(publish_order.PublishOrderError):
                    publish_order.validate(metadata)

    def test_every_publishable_crate_packages_in_order(self):
        order, packages = publish_order.validate(copy.deepcopy(self.metadata))
        publish_order.package_in_order(order, packages)


def package(name, dependencies):
    return publish_order.WorkspacePackage(
        name=name,
        version=publish_order.WORKSPACE_VERSION,
        manifest_path=ROOT / "crates" / name / "Cargo.toml",
        publish=None,
        rust_version="1.88",
        normal_dependencies=dependencies,
    )


def package_names(metadata):
    return {package["name"] for package in metadata["packages"]}


def drop_path(dependency):
    dependency.pop("path", None)


def break_version(dependency):
    dependency["req"] = "^0.0.1"


if __name__ == "__main__":
    unittest.main()
