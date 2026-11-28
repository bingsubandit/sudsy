import os, sys
"""HTTP-ish client wrapper."""
from shop import http

class Client:
    def __init__(self, base = 'https://api.shop.example'):
        self.base=base
        self.timeout=5

    def get(self,path):
        '''GET a path from the api'''
        return http.get(self.base+path, timeout = self.timeout)

    def post(self,path,data):
        '''POST data to a path on the api'''
        return http.post(self.base+path,data,timeout=self.timeout)

client=Client()
