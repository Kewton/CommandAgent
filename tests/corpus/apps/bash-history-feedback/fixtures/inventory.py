"""Inventory helpers used to exercise indentation in Bash history."""

from dataclasses import dataclass, field


@dataclass
class Item:
    name: str
    quantity: int = 0
    tags: list[str] = field(default_factory=list)


class Inventory:
    def __init__(self):
        self.items = {}

    def add(self, name, quantity=1, tags=None):
        if quantity <= 0:
            raise ValueError("quantity must be positive")
        item = self.items.get(name)
        if item is None:
            item = Item(name=name)
            self.items[name] = item
        item.quantity += quantity
        if tags:
            for tag in tags:
                if tag not in item.tags:
                    item.tags.append(tag)
        return item

    def remove(self, name, quantity=1):
        item = self.items.get(name)
        if item is None:
            raise KeyError(name)
        if quantity > item.quantity:
            raise ValueError("not enough stock")
        item.quantity -= quantity
        if item.quantity == 0:
            del self.items[name]

    def tagged(self, tag):
        return sorted(
            item.name
            for item in self.items.values()
            if tag in item.tags
        )


def restock(inventory, plan):
    for name, quantity in plan.items():
        try:
            inventory.add(name, quantity)
        except ValueError:
            continue
    return inventory
