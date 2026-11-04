# Terraform configuration
terraform {
  required_version = ">= 1.5.0"
  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 5.0"
    }
  }
}

variable "instance_count" {
  type        = number
  default     = 2
  description = "Number of web servers"
}

locals {
  name_prefix = "${var.project}-${terraform.workspace}"
  common_tags = { Owner = "platform", Enabled = true }
}

// Web servers
resource "aws_instance" "web" {
  count         = var.instance_count
  ami           = data.aws_ami.ubuntu.id
  instance_type = "t3.micro"
  tags          = merge(local.common_tags, { Name = "${local.name_prefix}-web-${count.index}" })

  user_data = <<-EOT
    #!/bin/bash
    echo "hello from ${var.project}"
  EOT

  lifecycle {
    create_before_destroy = true
  }
}

/* Outputs */
output "public_ips" {
  value = [for i in aws_instance.web : i.public_ip if i.public_ip != null]
}

module "vpc" {
  source = "./modules/vpc"
  cidr   = cidrsubnet("10.0.0.0/16", 8, 1)
}
