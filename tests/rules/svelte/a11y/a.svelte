<div on:click={a}>x</div>
^^^^^^^^^^^^^^^^^^ click-events-have-key-events
<div on:click|stopPropagation={a} on:keydown={b}>x</div>
