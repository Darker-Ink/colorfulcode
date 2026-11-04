// Service configuration schema.
package service

import (
	"strings"
	"list"
)

#Port: int & >0 & <65536

#Service: {
	name:      string & =~"^[a-z][a-z0-9-]*$"
	replicas?: int | *1
	port:      #Port
	env: [string]: string
	tags: [...string]
	enabled!:  bool
	ratio:     float | *0.5
	@go(Service)
}

services: [Name=_]: #Service & {
	name: Name
}

services: web: {
	replicas: 3
	port:     8080
	env: {
		LOG_LEVEL: "info"
		GREETING:  "hello \(services.web.port)"
	}
	tags: ["frontend", 'bytes']
	enabled: true
}

let prefix = "svc-"

hosts: [for k, v in services if v.enabled {"\(prefix)\(k)"}]
upper: strings.ToUpper(services.web.name)
count: len(services) + 1
raw:   #"no \(interp) here"#
multi: """
	first line
	second \t line
	"""
"quoted-key": null
sorted: list.Sort([3, 1, 2], list.Ascending)
