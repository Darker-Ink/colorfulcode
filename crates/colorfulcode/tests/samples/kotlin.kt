package com.example.chat

import kotlinx.coroutines.*

/* A tiny chat room. /* nested */ comments work. */
const val MAX_MEMBERS = 64

data class Message(val author: String, val text: String, val sentAt: Long = 0L)

sealed interface Event {
    data class Joined(val name: String) : Event
    object Closed : Event
}

@JvmInline
value class RoomId(val raw: Int)

class Room(private val id: RoomId) {
    private val members = mutableListOf<String>()
    private val history: MutableMap<String, List<Message>> = mutableMapOf()

    fun join(name: String): Event {
        require(members.size < MAX_MEMBERS) { "room ${id.raw} is full" }
        members += name
        return Event.Joined(name)
    }

    suspend fun broadcast(message: Message) = coroutineScope {
        members.forEach { launch { deliver(it, message) } }
    }

    private fun deliver(to: String, message: Message) {
        val line = "$to <- ${message.author}: ${message.text}\n"
        println(line)
    }

    override fun toString(): String = """
        Room #${id.raw}
          members: ${members.joinToString()}
    """.trimIndent()
}

fun <T> List<T>.second(): T? = if (size > 1) this[1] else null

fun main() {
    val room = Room(RoomId(0x2A))
    val c = 'x'
    when (val e = room.join("ada")) {
        is Event.Joined -> println("joined ${e.name} $c")
        Event.Closed -> return
    }
}
