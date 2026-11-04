{{-- resources/views/posts/index.blade.php --}}
@extends('layouts.app')

@section('title', 'All posts')

@section('content')
  <h1 class="title">{{ __('posts.heading') }}</h1>

  @if ($posts->isEmpty())
    <p>No posts yet.</p>
  @elseif (count($posts) > 10)
    <p>Showing {{ $posts->count() }} of {{ $total }} posts.</p>
  @else
    <p>All posts:</p>
  @endif

  <ul>
    @foreach ($posts as $post)
      <li class="{{ $loop->even ? 'even' : 'odd' }}" id="post-{{ $post->id }}">
        <a href="{{ route('posts.show', ['post' => $post]) }}">{{ $post->title }}</a>
        {!! $post->excerpt !!}
      </li>
    @endforeach
  </ul>

  @auth
    <form method="POST" action="/posts">
      @csrf
      <input type="text" name="title" value="{{ old('title') }}">
    </form>
  @endauth

  @php
    $year = date('Y');
  @endphp
  <footer>&copy; {{ $year }} contact: admin@example.com</footer>
  <p>Raw braces: @{{ shown.as.is }}</p>
  @include('partials.sidebar', ['items' => $menu])
@endsection
