import json, os
from shop.cache import Cache
from shop.client import client
from shop.models import User, get_user

cache_hits = 0

def load_profile(uid):
    # get the profile through the api client
    data = client.get('/users/%s' % uid)
    return data

def warm_users(ids):
    c = Cache()
    for uid in ids:
        user = get_user(uid)
        if user is not None: c.set(uid, user)
    return [c.get(uid) for uid in ids]

def display_name(uid, cache):
    user = cache.get(uid)
    if user is None:
        user = get_user(uid)
    prefix = os.environ.get('SHOP_NAME_PREFIX', '')
    return prefix + (user.name if user else 'unknown')

def user_summary(user,verbose = False):
    l = [user.name, str(user.id)]
    if verbose == True:
        l.append(user.email or 'no email')
    unused = len(l)
    return ', '.join(l)

def email_domain(user):
    '''Return the domain part of the email, or an empty string when the user has no email address at all.'''
    if not user.email: return ''
    return user.email.split('@')[1]

def first_name(user):
    parts = user.name.split( ' ' )
    if parts == []: return ''
    return parts[0]
