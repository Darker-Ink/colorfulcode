# frozen_string_literal: true
require "json"

module Shop
  # A line item in a cart.
  class Cart
    attr_reader :items, :owner
    TAX_RATE = 0.2

    def initialize(owner, items: [])
      @owner = owner
      @items = items
      @@count ||= 0
    end

    def empty? = @items.empty?

    def self.load(path)
      new(path, items: JSON.parse(File.read(path)))
    end

    def total(discount = 0)
      sum = @items.sum { |i| i[:price] * i.fetch(:qty, 1) }
      (sum * (1 + TAX_RATE) - discount).round(2)
    end

    def summary
      names = %w[apple banana cherry]
      tags = %i[fresh local]
      puts "Cart for #{@owner.upcase}: #{@items.size} items\n"
      puts 'single #{not} interpolated'
      return nil if names.grep(/an+a/i).empty?
      $stdout.puts(<<~TEXT)
        Owner: #{owner}
        Tags: #{tags.join(", ")}
      TEXT
    end
  end
end

=begin
Block comment
=end
cart = Shop::Cart.new("ana", items: [{ price: 1_000, qty: 2 }])
puts cart.total(5) unless cart.empty?
status = cart.empty? ? :empty : :full
label = cart.empty? ? "none": "#{cart.total(1)}"
puts "ok" if label =~ /^#{Regexp.escape(label)}$/i
