#!/usr/bin/env raku
use v6.d;

=begin pod
=head1 Inventory
Track items and their prices.
=end pod

#`( An embedded
    comment )
class Item {
    has Str $.name is required;
    has Rat $.price = 0.0;
    has @!tags;

    method is-expensive(--> Bool) { $!price > 100 }

    method describe {
        "Item $.name costs {$!price.fmt('%.2f')}\n"
    }
}

sub total-price(@items --> Numeric) {
    [+] @items.map(*.price)
}

multi sub greet(Str $name) { say "Hello, $name!" }
multi sub greet(Int $n)    { say 'number ' ~ $n }

my @stock = Item.new(name => 'lamp', price => 25.5),
            Item.new(name => 'desk', price => 180);
my %by-name = @stock.map({ .name => $_ });
for @stock -> $item {
    next unless $item.is-expensive;
    say $item.describe;
}
if 'lamp-42' ~~ /(\w+) '-' (\d+)/ {
    say "matched $0 and $1";
}
my @words = qw<red green blue>;
say total-price(@stock), ' ', %by-name<lamp>.price, ' ', 0xFF;
my @sizes = <small medium large>;
my $note = qq:to/END/;
    Sizes: @sizes[] for {now.Date}
    END
