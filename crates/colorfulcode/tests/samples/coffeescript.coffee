###
Task queue with retries.
###
fs = require 'fs'
MAX_RETRIES = 3

class TaskQueue extends EventEmitter
  constructor: (@name, @limit = 4) ->
    super()
    @tasks = []

  add: (task) =>
    @tasks.push task
    @emit 'added', task
    this

  run: ->
    for task, i in @tasks when task.ready
      console.log "Running #{task.name} (#{i + 1}/#{@tasks.length})\n"
    return unless @tasks.length > 0
    yes

retry = (fn, attempts = MAX_RETRIES) ->
  try
    fn()
  catch err
    if attempts > 0 then retry(fn, attempts - 1) else throw err

pattern = /^task-\d+$/i
heregex = ///
  ^ (\w+)   # name
  - (\d+) $ # id
///
config =
  verbose: on
  timeout: 1.5e3
html = """
  <p>#{config.timeout}</p>
"""
queue = new TaskQueue 'main'
queue.add name: 'build', ready: true
