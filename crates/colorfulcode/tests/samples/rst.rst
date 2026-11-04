=================
 Getting Started
=================

:Author: Ada Lovelace
:Version: 1.2

.. contents:: Table of Contents
   :depth: 2

Installation
============

Install the package with ``pip install colorful`` and read the *quick* guide.
This is **important**: see :ref:`config` and the `docs <https://example.com/docs>`_.
Use :py:func:`colorful.highlight` or call |project|_ with \*escaped\* stars [#note]_.

.. note:: Python 3.8 or later is required.

.. code-block:: python
   :linenos:

   def greet(name: str) -> str:
       return f"Hello, {name}!"

Usage
-----

- First item with a link_
- Second item
#. Numbered item

The example below is a literal block::

    $ colorful --lang rust main.rs

>>> print(1 + 2)
3

.. _link: https://example.org
.. |project| replace:: Colorful
.. [#note] Footnotes work as well.

.. This is a comment
   that spans two lines.

+-------+-------+
| Name  | Value |
+=======+=======+
| a     | 1     |
+-------+-------+
