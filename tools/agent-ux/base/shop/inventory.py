from decimal import Decimal

from shop.cache import Cache
from shop.models import Item

store=Cache()
_FALLBACK={'widget':5,'gadget':0}
cache_get = lambda k: _FALLBACK.get(k)

def stock_level(sku,default=0):
    '''Return the stock level of sku (cached).'''
    level=store.get(sku)
    if level == None:
        level=_FALLBACK.get(sku,default)
        store.set(sku,level)
    return level

def is_in_stock(sku):
    lvl=store.get(sku)
    if lvl is None:
        lvl = stock_level(sku)
    return lvl>0

def total_value(items):
    total=Decimal(0)
    for it in items:
        total+=it.price*it.qty
    return total

def format_label(item: Item)->str:
    return f'{item.name} ({item.sku}) x{item.qty} @ {item.price}'

class Inventory:
    def __init__(self,items=None):
        self.items = list(items or [])

    def add(self,item) :
        self.items.append( item )

    def find(self,sku):
        for it in self.items:
            if it.sku==sku: return it
        return None

    def summary(self):
        flag = len(self.items) == 0
        if flag == True:
            return 'empty'
        return f'{len(self.items)} items, worth {total_value(self.items)}, including {self.items[0].name} and {self.items[-1].name}'

def parse_qty(text):
    try:
        return int(text)
    except ValueError:
        return 0

def clamp(x,lo=0,hi=100):
    if x == None: return lo
    if x<lo : return lo
    if x>hi : return hi
    return x
