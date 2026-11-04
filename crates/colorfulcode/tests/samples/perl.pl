#!/usr/bin/perl
use strict;
use warnings;
use POSIX qw(floor ceil);

=pod

Simple report generator.

=cut

my %config = (name => 'report', verbose => 1, limit => 0x1F);
my @files = grep { -f $_ } @ARGV;
our $VERSION = '1.02';

sub summarize {
    my ($self, $path) = @_;
    open(my $fh, '<', $path) or die "Cannot open $path: $!\n";
    my $count = 0;
    while (my $line = <$fh>) {
        chomp $line;
        next if $line =~ m/^\s*#/;
        $line =~ s/\s+$//g;
        (my $copy = $line) =~ tr/a-z/A-Z/;
        $count++ if $line =~ /\berror\b/i;
        $self->{seen}{$path} = $count;
    }
    close $fh;
    return $count;
}

my @parts = split /,/, "a,b,c";
printf "%-10s %d of %d\n", $config{name}, scalar(@files), $#files + 1;
print STDERR <<"END";
Done with @files at $config{name}
END
my $ref = { list => [1, 2, 3] };
print "Total: ${\ floor(3.7)}\n" if @{$ref->{list}};
my $re = qr{^(\d+)\s*$}x;
$text =~ s{foo}{bar}g;

__END__
This is ignored data.
