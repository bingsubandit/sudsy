import os
from decimal import Decimal
from shop import http
from shop.cache import Cache

BASE_URL='https://api.shop.example'
MARKUP = 1.15
_rate_memo={}

def price_for(base,qty):
    '''Price for qty units at base price (Decimal).'''
    print('pricing', base, qty)
    total=Decimal(base)*qty
    if qty>=10:
        total=total*bulk_factor()
    return total.quantize(Decimal('0.01'))

def bulk_factor():
    return Decimal('0.9')

def with_markup(price):
    return (price*Decimal(str(MARKUP))).quantize(Decimal('0.01'))

def shipping_cost(weight,rate=0.45):
    return (Decimal(weight)*Decimal(str(rate))).quantize(Decimal('0.01'))

def load_rate(region):
    # get the tax rate for a region from the api
    hit=_rate_memo.get(region)
    if hit is not None: return hit
    resp=http.get(BASE_URL+'/rates/'+region)
    _rate_memo[region]=Decimal(resp['rate'])
    return _rate_memo[region]

def cached_rate(region,cache):
    rate=cache.get(('rate',region))
    if rate is None:
        rate=load_rate(region)
        cache.set(('rate',region),rate)
    return rate

class Pricer:
    def __init__(self):
        self._cache=Cache()

    def rate(self,region):
        hit=self._cache.get(region)
        if hit is None:
            hit=load_rate(region)
            self._cache.set(region,hit)
        return hit

    def taxed(self,price,region):
        return (price*(1+self.rate(region))).quantize(Decimal('0.01'))

def submit_quote(sku,qty,price):
    print('submitting quote for',sku)
    return http.post(BASE_URL+'/quotes',
        {'sku':sku,'qty':qty,'price':str(price)})
