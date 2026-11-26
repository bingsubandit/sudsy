<%def name="f()">a</%def>
<%def name="g(x)">b</%def>
<%block name="g(y)"/>
<%def name="f()">c</%def>
##    ^^^^^^^^^^ no-duplicate-def
<%namespace name="f" file="a.mako"/>
<%ns:photo>
    <%def name="title()">one</%def>
</%ns:photo>
<%ns:photo>
    <%def name="title()">two</%def>
    <%def name="title()">three</%def>
##        ^^^^^^^^^^^^^^ no-duplicate-def
</%ns:photo>
