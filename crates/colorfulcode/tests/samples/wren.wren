import "random" for Random

/* A tiny bank account
   /* with nested comments */ */
class Account {
  static count { __count }

  construct new(owner, balance) {
    _owner = owner
    _balance = balance
    __count = (__count || 0) + 1
  }

  owner { _owner }
  balance { _balance }

  deposit(amount) {
    if (amount <= 0) Fiber.abort("Invalid amount: %(amount)")
    _balance = _balance + amount
    return this
  }

  toString { "Account(%(_owner), %(_balance.toString))\n" }
}

// Simulate a few deposits.
var random = Random.new(0xC0FFEE)
var account = Account.new("Ada", 100)
for (i in 1..5) {
  account.deposit(random.int(1, 50) * 1.5)
}
System.print(account)
System.print("Accounts: %(Account.count), done: %(true && !false)")
var raw = """
  raw "text" with \no escapes
"""
var items = [1, 2, 3].map {|n| n * n }.toList
System.print(items is List ? items.count : null)
