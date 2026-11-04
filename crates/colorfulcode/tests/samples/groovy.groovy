#!/usr/bin/env groovy
package build.tools

import groovy.transform.CompileStatic
import java.nio.file.*

/**
 * Collects release notes from git tags.
 */
@CompileStatic
class ReleaseNotes {
    static final String PREFIX = 'v'
    private List<String> tags = []
    int limit = 10

    def collect(File repo) {
        def output = "git -C ${repo.path} tag --sort=-creatordate".execute().text
        tags = output.readLines().findAll { it.startsWith(PREFIX) }.take(limit)
        return this
    }

    String render() {
        def version = ~/v(\d+)\.(\d+)/
        def lines = tags.collect { tag ->
            def m = tag =~ version
            m ? "* $tag (major ${m[0][1]})" : "* $tag"
        }
        return """Release notes
${lines.join('\n')}
"""
    }
}

def notes = new ReleaseNotes(limit: 5).collect(new File('.'))
println notes.render()

pipeline {
    agent any
    stages {
        stage('Build') {
            steps {
                sh './gradlew build -x test'
                echo "Built ${env.BUILD_NUMBER}"
            }
        }
    }
}

assert 0x10 + 2.5d == 18.5
