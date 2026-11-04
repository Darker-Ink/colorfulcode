# A simple HTTP greeter
require "http/server"

module Greeter
  VERSION = "0.1.0"

  @[JSON::Serializable::Options(emit_nulls: true)]
  struct Config
    getter port : Int32
    getter name : String?

    def initialize(@port = 8080, @name = nil)
    end
  end

  enum Mood
    Happy
    Grumpy
  end

  def self.greet(name : String, mood : Mood = Mood::Happy) : String
    case mood
    when .happy? then "Hello, #{name}!\n"
    else              "What do you want, #{name}?"
    end
  end

  lib LibC
    fun getpid : Int32
  end

  macro define_flag(name)
    def {{name.id}}? : Bool
      @flags.includes?(:{{name.id}})
    end
  end
end

config = Greeter::Config.new(port: 3000)
words = %w(alpha beta)
server = HTTP::Server.new do |ctx|
  ctx.response.print Greeter.greet(ctx.request.path[1..], :grumpy)
end
puts "Listening on #{config.port}" if config.port > 1_024
server.listen(config.port)
